use std::net::{AddrParseError, SocketAddr};

use rhai::{CustomType, Engine, EvalAltResult, TypeBuilder, AST};
use specht_core::{endpoint::Endpoint, io::Io, Result};

type HandlerName = String;

#[derive(Debug, Clone, PartialEq)]
pub enum AcceptorConfig {
    Socks5(SocketAddr, HandlerName),
    Http(SocketAddr, HandlerName),
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstanceConfig {
    pub acceptors: Vec<AcceptorConfig>,
}

impl InstanceConfig {
    pub fn new() -> Self {
        Self { acceptors: vec![] }
    }

    pub fn add_socks5_acceptor(
        &mut self,
        addr: &str,
        handler_name: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        self.acceptors.push(AcceptorConfig::Socks5(
            addr.parse().map_err(|e: AddrParseError| e.to_string())?,
            handler_name.to_owned(),
        ));

        Ok(())
    }

    pub fn add_http_acceptor(
        &mut self,
        addr: &str,
        handler_name: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        self.acceptors.push(AcceptorConfig::Http(
            addr.parse().map_err(|e: AddrParseError| e.to_string())?,
            handler_name.to_owned(),
        ));

        Ok(())
    }
}

impl CustomType for InstanceConfig {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("InstanceConfig")
            .with_fn("new_config", Self::new)
            .with_fn("add_socks5", Self::add_socks5_acceptor)
            .with_fn("add_http", Self::add_http_acceptor);
    }
}
pub struct ConfigEngine {
    engine: Engine,
    ast: AST,
}

impl ConfigEngine {
    pub fn compile_config(code: impl AsRef<str>) -> Result<ConfigEngine> {
        let mut engine = Engine::new();

        engine.build_type::<InstanceConfig>();

        let ast = engine.compile(code)?;

        Ok(Self { engine, ast })
    }

    pub fn eval_config(&self) -> Result<InstanceConfig> {
        self.engine.eval_ast(&self.ast).map_err(Into::into)
    }

    pub async fn run_handler(
        &self,
        _name: String,
        _endpoint: Endpoint,
    ) -> Result<Box<dyn Io + Send>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_acceptor() {
        let engine = ConfigEngine::compile_config(
            r#"
            let config = new_config();

            config.add_socks5("127.0.0.1:8080", "handler");
            config.add_http("127.0.0.1:8081", "handler");

            config
        "#,
        )
        .unwrap();

        assert_eq!(
            engine.eval_config().unwrap().acceptors,
            vec![
                AcceptorConfig::Socks5("127.0.0.1:8080".parse().unwrap(), "handler".to_owned()),
                AcceptorConfig::Http("127.0.0.1:8081".parse().unwrap(), "handler".to_owned())
            ]
        );
    }
}
