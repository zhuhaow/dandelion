use std::{net::SocketAddr, sync::Arc};

use rune::{
    runtime::RuntimeContext,
    termcolor::{ColorChoice, StandardStream},
    Any, Context, Diagnostics, FromValue, Module, Source, Sources, Unit, Vm,
};
use specht_core::Result;

use crate::connector::{ConnectRequest, Connector};

type HandlerName = String;

#[derive(Debug, PartialEq)]
pub enum AcceptorConfig {
    Socks5(SocketAddr, HandlerName),
    Http(SocketAddr, HandlerName),
}

#[derive(Debug, PartialEq, Any)]
pub struct InstanceConfig {
    pub acceptors: Vec<AcceptorConfig>,
}

impl InstanceConfig {
    pub fn new() -> Self {
        Self { acceptors: vec![] }
    }

    pub fn add_socks5_acceptor(&mut self, addr: &str, handler_name: &str) -> Result<()> {
        self.acceptors.push(AcceptorConfig::Socks5(
            addr.parse()?,
            handler_name.to_owned(),
        ));

        Ok(())
    }

    pub fn add_http_acceptor(&mut self, addr: &str, handler_name: &str) -> Result<()> {
        self.acceptors
            .push(AcceptorConfig::Http(addr.parse()?, handler_name.to_owned()));

        Ok(())
    }
}

impl InstanceConfig {
    fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.function(["Config", "new"], Self::new)?;
        module.inst_fn("add_socks5", Self::add_socks5_acceptor)?;
        module.inst_fn("add_http", Self::add_http_acceptor)?;

        Ok(module)
    }
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ConfigEngine {
    context: Arc<RuntimeContext>,
    unit: Arc<Unit>,
}

impl ConfigEngine {
    pub fn compile_config(name: impl AsRef<str>, code: impl AsRef<str>) -> Result<ConfigEngine> {
        let mut sources = Sources::new();
        sources.insert(Source::new(name, code));

        let mut context = Context::with_default_modules()?;
        context.install(InstanceConfig::module()?)?;
        context.install(ConnectRequest::module()?)?;
        context.install(Connector::module()?)?;

        let mut diagnostics = Diagnostics::new();
        let result = rune::prepare(&mut sources)
            .with_context(&context)
            .with_diagnostics(&mut diagnostics)
            .build();

        if !diagnostics.is_empty() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;
        }

        Ok(Self {
            context: Arc::new(context.runtime()),
            unit: Arc::new(result?),
        })
    }

    fn vm(&self) -> Vm {
        Vm::new(self.context.clone(), self.unit.clone())
    }

    pub fn eval_config(&self) -> Result<InstanceConfig> {
        Ok(InstanceConfig::from_value(self.vm().call(["config"], ())?)?)
    }

    pub fn run_handler(&self, name: impl AsRef<str>, request: ConnectRequest) -> Result<Connector> {
        Ok(Connector::from_value(
            self.vm().call([name.as_ref()], (request,))?,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use specht_core::endpoint::Endpoint;

    use super::*;

    #[test]
    fn test_add_acceptor() -> Result<()> {
        let engine = ConfigEngine::compile_config(
            "config",
            r#"
            pub fn config() {
                let config = Config::new();

                config.add_socks5("127.0.0.1:8080", "handler");
                config.add_http("127.0.0.1:8081", "handler");

                config
            }
        "#,
        )?;

        assert_eq!(
            engine.eval_config()?.acceptors,
            vec![
                AcceptorConfig::Socks5("127.0.0.1:8080".parse()?, "handler".to_owned()),
                AcceptorConfig::Http("127.0.0.1:8081".parse()?, "handler".to_owned())
            ]
        );

        Ok(())
    }

    #[rstest]
    #[case(
        r#"
            pub fn handler(request) {
                Connector::tcp(request.endpoint(), "id")
            }
        "#,
        "example.com:80",
        Connector::tcp("example.com:80", "id")
    )]
    fn test_connect_request_bridge(
        #[case] config: &str,
        #[case] endpoint: Endpoint,
        #[case] expect: Connector,
    ) -> Result<()> {
        let engine = ConfigEngine::compile_config("config", config)?;

        let connector = engine.run_handler("handler", endpoint.into())?;

        assert_eq!(connector, expect);

        Ok(())
    }
}
