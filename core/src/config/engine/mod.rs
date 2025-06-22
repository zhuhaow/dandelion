mod connect;
mod geoip;
mod iplist;
mod resolver;
mod testing;

use self::{
    connect::{ConnectRequest, IoWrapper},
    geoip::GeoIp,
    iplist::IpNetworkSetWrapper,
    resolver::ResolverWrapper,
};
use crate::{
    core::{
        acceptor::{http, socks5},
        endpoint::Endpoint,
        io::Io,
    },
    Result,
};
use anyhow::Context as AnyhowContext;
use futures::{future::select_all, Future, FutureExt};
use rune::{
    alloc::clone::TryClone,
    runtime::{Object, RuntimeContext},
    termcolor::{ColorChoice, StandardStream},
    Any, Context, Diagnostics, Module, Source, Sources, Unit, Vm,
};
use std::{net::SocketAddr, rc::Rc, sync::Arc};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};

type HandlerName = String;

#[derive(Debug, PartialEq)]
pub enum AcceptorConfig {
    Socks5(SocketAddr, HandlerName),
    Http(SocketAddr, HandlerName),
}

#[derive(Debug, Any)]
struct Config {
    acceptors: Vec<AcceptorConfig>,
    #[rune(get, set)]
    cache: Option<Object>,
}

impl Config {
    #[rune::function(path = Self::new)]
    pub fn new() -> Self {
        Self {
            acceptors: Vec::new(),
            cache: None,
        }
    }

    #[rune::function]
    pub fn add_socks5_acceptor(&mut self, addr: &str, handler_name: &str) -> Result<()> {
        self.acceptors.push(AcceptorConfig::Socks5(
            addr.parse()?,
            handler_name.to_owned(),
        ));

        Ok(())
    }

    #[rune::function]
    pub fn add_http_acceptor(&mut self, addr: &str, handler_name: &str) -> Result<()> {
        self.acceptors
            .push(AcceptorConfig::Http(addr.parse()?, handler_name.to_owned()));

        Ok(())
    }
}

impl Config {
    fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.function_meta(Self::new)?;
        module.function_meta(Self::add_socks5_acceptor)?;
        module.function_meta(Self::add_http_acceptor)?;

        Ok(module)
    }
}

pub struct Engine {
    context: Arc<RuntimeContext>,
    unit: Arc<Unit>,
    acceptors: Vec<AcceptorConfig>,
    cache: Option<Object>,
}

impl Engine {
    pub async fn load_config(code: impl AsRef<str>) -> Result<Engine> {
        let mut sources = Sources::new();
        sources.insert(Source::memory(code)?)?;

        let mut context = Context::with_default_modules()?;
        context.install(Config::module()?)?;
        context.install(ConnectRequest::module()?)?;
        context.install(ResolverWrapper::module()?)?;
        context.install(IpNetworkSetWrapper::module()?)?;
        context.install(GeoIp::module()?)?;

        let mut diagnostics = Diagnostics::new();
        let result = rune::prepare(&mut sources)
            .with_context(&context)
            .with_diagnostics(&mut diagnostics)
            .build();

        if !diagnostics.is_empty() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;
        }

        let context = Arc::new(context.runtime()?);
        let unit = Arc::new(result?);

        let mut vm = Vm::new(context.clone(), unit.clone());

        log::info!("Configuring rule engine...");

        let config: Config =
            rune::from_value::<Result<Config>>(vm.async_call(["config"], ()).await?)??;

        // Everything should be clonable, but we check it here to ensure
        if let Some(c) = config.cache.as_ref() {
            c.try_clone()
                .context("Cache can only contain cloneable objects")?;
        }

        log::info!("Done");

        Ok(Self {
            context,
            unit,
            acceptors: config.acceptors,
            cache: config.cache,
        })
    }

    fn vm(&self) -> Vm {
        Vm::new(self.context.clone(), self.unit.clone())
    }

    pub async fn handle_acceptors<
        F: Future<Output = Result<(Endpoint, impl Future<Output = Result<impl Io>>)>> + 'static,
    >(
        self: Rc<Self>,
        addr: &SocketAddr,
        handshake: fn(TcpStream) -> F,
        eval_fn: String,
    ) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;

        loop {
            let io = listener.accept().await?.0;

            let engine = self.clone();
            let eval_fn = eval_fn.clone();

            tokio::task::spawn_local(async move {
                if let Err(e) = async move {
                    let (endpoint, fut) = handshake(io).await?;

                    let endpoint_cloned = endpoint.clone();
                    async move {
                        let mut remote = rune::from_value::<Result<IoWrapper>>(
                            engine
                                .vm()
                                .async_call(
                                    [eval_fn.as_str()],
                                    (ConnectRequest::new(endpoint), engine.cache.try_clone()?),
                                )
                                .await?,
                        )??
                        .into_inner();

                        let mut local = fut.await?;

                        copy_bidirectional(&mut local, &mut remote)
                            .await
                            .context("Error happened when forwarding data")?;

                        anyhow::Ok(())
                    }
                    .await
                    .with_context(|| format!("target endpoint {}", endpoint_cloned))
                }
                .await
                {
                    tracing::error!("{:?}", e)
                }
            });
        }
    }

    pub async fn run(self) -> Result<()> {
        let self_ptr = Rc::new(self);

        select_all(self_ptr.clone().acceptors.iter().map(|c| {
            match c {
                AcceptorConfig::Socks5(addr, handler) => self_ptr
                    .clone()
                    .handle_acceptors(addr, socks5::handshake, handler.to_owned())
                    .boxed_local(),
                AcceptorConfig::Http(addr, handler) => self_ptr
                    .clone()
                    .handle_acceptors(addr, http::handshake, handler.to_owned())
                    .boxed_local(),
            }
        }))
        .await
        .0
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_add_acceptor() -> Result<()> {
        let engine = Engine::load_config(
            r#"
            pub async fn config() {
                let config = Config::new();

                config.add_socks5_acceptor("127.0.0.1:8080", "handler")?;
                config.add_http_acceptor("127.0.0.1:8081", "handler")?;

                Ok(config)
            }
        "#,
        )
        .await?;

        assert_eq!(
            engine.acceptors,
            vec![
                AcceptorConfig::Socks5("127.0.0.1:8080".parse().unwrap(), "handler".to_owned()),
                AcceptorConfig::Http("127.0.0.1:8081".parse().unwrap(), "handler".to_owned())
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_add_cache() -> Result<()> {
        let engine = Engine::load_config(
            r#"
            pub async fn config() {
                let config = Config::new();
                config.cache = Some(#{
                    "key": "value"
                });
                Ok(config)
            }
        "#,
        )
        .await?;

        assert!(engine.cache.is_some());

        let cache = engine.cache.unwrap();

        assert!(cache.get("key").is_some());
        assert_eq!(
            rune::from_value::<String>(cache.get("key").unwrap())?,
            "value"
        );

        Ok(())
    }
}
