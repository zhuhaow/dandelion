mod connect;
mod geoip;
mod iplist;
mod resolver;

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

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
    runtime::{RuntimeContext, VmSendExecution},
    termcolor::{ColorChoice, StandardStream},
    Any, Context, Diagnostics, Module, Source, Sources, Unit, Vm,
};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};

use self::{
    connect::{ConnectRequest, IoWrapper, QuicConnectionWrapper},
    geoip::GeoIp,
    iplist::IpNetworkSetWrapper,
    resolver::ResolverWrapper,
};

type HandlerName = String;

#[derive(Debug, PartialEq)]
pub enum AcceptorConfig {
    Socks5(SocketAddr, HandlerName),
    Http(SocketAddr, HandlerName),
}

#[derive(Debug, Any, Clone)]
struct Cache {
    resolvers: HashMap<String, ResolverWrapper>,
    iplist: HashMap<String, IpNetworkSetWrapper>,
    geoip: Option<GeoIp>,
    quic_connections: HashMap<String, QuicConnectionWrapper>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            resolvers: HashMap::new(),
            iplist: HashMap::new(),
            geoip: None,
            quic_connections: HashMap::new(),
        }
    }

    pub fn get_resolver(&self, name: &str) -> Result<ResolverWrapper> {
        self.resolvers
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("resolver {} not found", name))
    }

    pub fn get_iplist(&self, name: &str) -> Result<IpNetworkSetWrapper> {
        self.iplist
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("iplist {} not found", name))
    }

    pub fn get_geoip_db(&self) -> Result<GeoIp> {
        self.geoip
            .clone()
            .ok_or_else(|| anyhow::anyhow!("geoip db not found"))
    }

    pub fn get_quic_connection(&self, name: &str) -> Result<QuicConnectionWrapper> {
        self.quic_connections
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("quic connection {} not found", name))
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.associated_function("try_get_resolver", Self::get_resolver)?;
        module.associated_function("try_get_iplist", Self::get_iplist)?;
        module.associated_function("try_get_geoip_db", Self::get_geoip_db)?;
        module.associated_function("try_get_quic_connection", Self::get_quic_connection)?;

        Ok(module)
    }
}

#[derive(Debug, Any)]
struct Config {
    acceptors: Vec<AcceptorConfig>,
    cache: Cache,
}

impl Config {
    #[rune::function(path = Self::new)]
    pub fn new() -> Self {
        Self {
            acceptors: Vec::new(),
            cache: Cache::new(),
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

    #[rune::function]
    pub fn cache_resolver(&mut self, name: &str, resolver: ResolverWrapper) {
        self.cache.resolvers.insert(name.to_owned(), resolver);
    }

    #[rune::function]
    pub fn cache_iplist(&mut self, name: &str, iplist: IpNetworkSetWrapper) {
        self.cache.iplist.insert(name.to_owned(), iplist);
    }

    #[rune::function]
    pub fn cache_geoip_db(&mut self, db: GeoIp) {
        self.cache.geoip = Some(db);
    }

    #[rune::function]
    pub fn cache_quic_connection(&mut self, name: &str, connection: QuicConnectionWrapper) {
        self.cache
            .quic_connections
            .insert(name.to_owned(), connection);
    }
}

impl Config {
    fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.function_meta(Self::new)?;
        module.function_meta(Self::add_socks5_acceptor)?;
        module.function_meta(Self::add_http_acceptor)?;
        module.function_meta(Self::cache_resolver)?;
        module.function_meta(Self::cache_iplist)?;
        module.function_meta(Self::cache_geoip_db)?;
        module.function_meta(Self::cache_quic_connection)?;

        Ok(module)
    }
}

pub struct Engine {
    context: Arc<RuntimeContext>,
    unit: Arc<Unit>,
    acceptors: Vec<AcceptorConfig>,
    cache: Cache,
}

impl Engine {
    pub async fn load_config(name: impl AsRef<str>, code: impl AsRef<str>) -> Result<Engine> {
        let mut sources = Sources::new();
        sources.insert(Source::new(name, code)?)?;

        let mut context = Context::with_default_modules()?;
        context.install(Config::module()?)?;
        context.install(ConnectRequest::module()?)?;
        context.install(ResolverWrapper::module()?)?;
        context.install(IpNetworkSetWrapper::module()?)?;
        context.install(Cache::module()?)?;
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

    pub fn create_handler_execution(
        &self,
        name: impl AsRef<str>,
        endpoint: Endpoint,
    ) -> Result<VmSendExecution> {
        Ok(self.vm().send_execute(
            [name.as_ref()],
            (ConnectRequest::new(endpoint), self.cache.clone()),
        )?)
    }

    pub async fn run(self) -> Result<()> {
        let self_ptr = Arc::new(self);

        select_all(self_ptr.clone().acceptors.iter().map(|c| {
            match c {
                AcceptorConfig::Socks5(addr, handler) => handle_acceptors(
                    addr,
                    socks5::handshake,
                    self_ptr.clone(),
                    handler.to_owned(),
                )
                .boxed(),
                AcceptorConfig::Http(addr, handler) => {
                    handle_acceptors(addr, http::handshake, self_ptr.clone(), handler.to_owned())
                        .boxed()
                }
            }
        }))
        .await
        .0
    }
}

pub async fn handle_acceptors<
    F: Future<Output = Result<(Endpoint, impl Future<Output = Result<impl Io>> + Send)>>
        + 'static
        + Send,
>(
    addr: &SocketAddr,
    handshake: fn(TcpStream) -> F,
    engine: Arc<Engine>,
    eval_fn: String,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;

    loop {
        let io = listener.accept().await?.0;

        let engine = engine.clone();
        let eval_fn = eval_fn.clone();

        tokio::task::spawn(async move {
            if let Err(e) = async move {
                let (endpoint, fut) = handshake(io).await?;

                let endpoint_cloned = endpoint.clone();
                async move {
                    let execution = engine.create_handler_execution(eval_fn, endpoint)?;

                    let mut remote = rune::from_value::<Result<IoWrapper>>(
                        execution
                            .async_complete()
                            .await
                            // a VmResult here
                            .into_result()?, // Unwrap it gives return value of the call,
                                             // the return value is of type `Value`, but it's actually a `Result`.
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

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_add_acceptor() -> Result<()> {
        let engine = Engine::load_config(
            "config",
            r#"
            pub async fn config() {
                let config = Config::new();

                config.add_socks5_acceptor("127.0.0.1:8080", "handler")?;
                config.add_http_acceptor("127.0.0.1:8081", "handler")?;

                config.cache_resolver("system", create_system_resolver()?);
                config.cache_resolver("google_dns", create_udp_resolver(["8.8.8.8:53"])?);

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
}
