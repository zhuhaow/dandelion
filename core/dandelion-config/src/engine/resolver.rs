use std::{net::SocketAddr, sync::Arc, time::Duration};

use dandelion_core::{
    resolver::{hickory::HickoryResolver, system::SystemResolver, Resolver},
    Result,
};
use hickory_resolver::config::{NameServerConfig, Protocol};
use rune::{
    runtime::{Ref, Vec as RuneVec},
    Any, FromValue, Module, ToValue, Value,
};

use crate::rune::create_wrapper;

create_wrapper!(ResolverWrapper, Resolver, Arc);

impl Clone for ResolverWrapper {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[rune::function]
fn create_system_resolver() -> Result<ResolverWrapper> {
    Ok(Arc::new(SystemResolver::default()).into())
}

#[rune::function]
fn create_udp_resolver(addrs: RuneVec) -> Result<ResolverWrapper> {
    Ok(Arc::new(HickoryResolver::new(
        addrs
            .into_iter()
            .map(|addr| {
                anyhow::Ok(
                    String::from_value(addr)
                        .into_result()?
                        .parse::<SocketAddr>()?,
                )
            })
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

        module.ty::<Self>()?;

        module.function_meta(create_system_resolver)?;
        module.function_meta(create_udp_resolver)?;

        module.function_meta(Self::lookup)?;
        module.function_meta(Self::lookup_ipv4)?;
        module.function_meta(Self::lookup_ipv6)?;

        Ok(module)
    }

    // See https://docs.rs/rune/latest/rune/struct.Module.html#method.function_meta
    // for why use `this` instead of `self`
    #[rune::function(instance, path = Self::lookup)]
    async fn lookup(this: Ref<Self>, hostname: Ref<str>) -> Result<RuneVec> {
        Ok(this
            .inner()
            .lookup_ip(hostname.as_ref())
            .await?
            .into_iter()
            .map(|ip| ip.to_string().to_value().into_result())
            .collect::<Result<Vec<Value>, _>>()?
            .try_into()?)
    }

    #[rune::function(instance, path = Self::lookup_ipv4)]
    async fn lookup_ipv4(this: Ref<Self>, hostname: Ref<str>) -> Result<RuneVec> {
        Ok(this
            .inner()
            .lookup_ipv4(hostname.as_ref())
            .await?
            .into_iter()
            .map(|ip| ip.to_string().to_value().into_result())
            .collect::<Result<Vec<Value>, _>>()?
            .try_into()?)
    }

    #[rune::function(instance, path = Self::lookup_ipv6)]
    async fn lookup_ipv6(this: Ref<Self>, hostname: Ref<str>) -> Result<RuneVec> {
        Ok(this
            .inner()
            .lookup_ipv6(hostname.as_ref())
            .await?
            .into_iter()
            .map(|ip| ip.to_string().to_value().into_result())
            .collect::<Result<Vec<Value>, _>>()?
            .try_into()?)
    }
}
