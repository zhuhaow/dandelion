use std::{net::SocketAddr, sync::Arc, time::Duration};

use rune::{
    runtime::{RuntimeContext, Vec as RuneVec, VmSendExecution},
    termcolor::{ColorChoice, StandardStream},
    Any, Context, Diagnostics, FromValue, Module, Source, Sources, Unit, Vm,
};
use specht_core::{endpoint::Endpoint, resolver::system::SystemResolver, Result};
use trust_dns_resolver::config::{NameServerConfig, Protocol};

use crate::{
    connector::{Connector, ResolverGroup},
    rune::value_to_result,
};

type HandlerName = String;

#[derive(Debug, PartialEq)]
pub enum AcceptorConfig {
    Socks5(SocketAddr, HandlerName),
    Http(SocketAddr, HandlerName),
}

#[derive(Debug, Any)]
pub struct InstanceConfig {
    pub acceptors: Vec<AcceptorConfig>,
    pub resolver_group: ResolverGroup,
}

impl InstanceConfig {
    pub fn new() -> Self {
        Self {
            acceptors: Vec::new(),
            resolver_group: ResolverGroup::new(),
        }
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

    pub fn add_system_resolver(&mut self, name: &str) -> Result<()> {
        self.resolver_group
            .add_resolver(name, Arc::new(SystemResolver::default()));

        Ok(())
    }

    pub fn add_udp_resolver(&mut self, name: &str, addrs: RuneVec) -> Result<()> {
        self.resolver_group.add_resolver(
            name,
            Arc::new(specht_core::resolver::trust::TrustResolver::new(
                addrs
                    .into_iter()
                    .map(|addr| anyhow::Ok(String::from_value(addr)?.parse()?))
                    .try_collect::<Vec<SocketAddr>>()?
                    .into_iter()
                    .map(|s| NameServerConfig::new(s, Protocol::Udp))
                    .collect(),
                Duration::from_secs(5),
            )?),
        );

        Ok(())
    }
}

impl InstanceConfig {
    fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.function(["Config", "new"], Self::new)?;
        module.inst_fn("add_socks5_acceptor", Self::add_socks5_acceptor)?;
        module.inst_fn("add_http_acceptor", Self::add_http_acceptor)?;
        module.inst_fn("add_system_resolver", Self::add_system_resolver)?;
        module.inst_fn("add_udp_resolver", Self::add_udp_resolver)?;

        Ok(module)
    }
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Engine {
    context: Arc<RuntimeContext>,
    unit: Arc<Unit>,
    acceptors: Vec<AcceptorConfig>,
    resolver_group: Arc<ResolverGroup>,
}

impl Engine {
    pub async fn load_config(name: impl AsRef<str>, code: impl AsRef<str>) -> Result<Engine> {
        let mut sources = Sources::new();
        sources.insert(Source::new(name, code));

        let mut context = Context::with_default_modules()?;
        context.install(InstanceConfig::module()?)?;
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

        let context = Arc::new(context.runtime());
        let unit = Arc::new(result?);

        let mut vm = Vm::new(context.clone(), unit.clone());

        let config: InstanceConfig =
            value_to_result(vm.async_call(["config"], ()).await?.into_result()?)?;

        Ok(Self {
            context,
            unit,
            acceptors: config.acceptors,
            resolver_group: Arc::new(config.resolver_group),
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
            (Connector::new(endpoint, self.resolver_group.clone()),),
        )?)
    }

    pub fn get_acceptors(&self) -> &[AcceptorConfig] {
        &self.acceptors
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

                config.add_socks5("127.0.0.1:8080", "handler")?;
                config.add_http("127.0.0.1:8081", "handler")?;

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
    async fn test_add_resolver() -> Result<()> {
        let engine = Engine::load_config(
            "config",
            r#"

            pub async fn config() {
                let config = Config::new();

                config.add_system_resolver("system")?;
                config.add_udp_resolver("udp", ["8.8.8.8:53", "114.114.114.114:53"])?;

                Ok(config)
            }
        "#,
        )
        .await?;

        assert!(engine.resolver_group.get_resolver("system").is_ok());

        assert!(engine.resolver_group.get_resolver("udp").is_ok());

        Ok(())
    }
}
