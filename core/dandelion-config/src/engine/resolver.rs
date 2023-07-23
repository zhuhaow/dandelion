use std::{net::SocketAddr, sync::Arc, time::Duration};

use dandelion_core::{
    resolver::{system::SystemResolver, trust::TrustResolver, Resolver},
    Result,
};
use rune::{runtime::Vec as RuneVec, Any, FromValue, Module, Value};
use trust_dns_resolver::config::{NameServerConfig, Protocol};

use crate::rune::create_wrapper;

create_wrapper!(ResolverWrapper, Resolver, Arc);

impl Clone for ResolverWrapper {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

fn create_system_resolver() -> Result<ResolverWrapper> {
    Ok(Arc::new(SystemResolver::default()).into())
}

fn create_udp_resolver(addrs: RuneVec) -> Result<ResolverWrapper> {
    Ok(Arc::new(TrustResolver::new(
        addrs
            .into_iter()
            .map(|addr| anyhow::Ok(String::from_value(addr)?.parse::<SocketAddr>()?))
            .try_fold(Vec::new(), |mut addrs, addr| {
                addrs.push(addr?);
                anyhow::Ok(addrs)
            })?
            .into_iter()
            .map(|s| NameServerConfig::new(s, Protocol::Udp))
            .collect(),
        Duration::from_secs(5),
    )?)
    .into())
}

impl ResolverWrapper {
    pub fn module() -> Result<Module> {
        let mut module = Module::new();

        module.function(&["try_create_system_resolver"], create_system_resolver)?;
        module.function(&["try_create_udp_resolver"], create_udp_resolver)?;

        module.ty::<Self>()?;
        module.async_inst_fn("try_lookup", Self::lookup)?;
        module.async_inst_fn("try_lookup_ipv4", Self::lookup_ipv4)?;
        module.async_inst_fn("try_lookup_ipv6", Self::lookup_ipv6)?;

        Ok(module)
    }

    async fn lookup(&self, hostname: &str) -> Result<RuneVec> {
        Ok(self
            .inner()
            .lookup_ip(hostname)
            .await?
            .into_iter()
            .map(|ip| Into::<Value>::into(ip.to_string()))
            .collect::<Vec<_>>()
            .into())
    }

    async fn lookup_ipv4(&self, hostname: &str) -> Result<RuneVec> {
        Ok(self
            .inner()
            .lookup_ipv4(hostname)
            .await?
            .into_iter()
            .map(|ip| Into::<Value>::into(ip.to_string()))
            .collect::<Vec<_>>()
            .into())
    }

    async fn lookup_ipv6(&self, hostname: &str) -> Result<RuneVec> {
        Ok(self
            .inner()
            .lookup_ipv6(hostname)
            .await?
            .into_iter()
            .map(|ip| Into::<Value>::into(ip.to_string()))
            .collect::<Vec<_>>()
            .into())
    }
}
