use crate::config::rune::create_wrapper;
use crate::{
    core::resolver::{hickory::HickoryResolver, system::SystemResolver, Resolver},
    Result,
};
use hickory_proto::xfer::Protocol;
use hickory_resolver::config::NameServerConfig;
use itertools::Itertools;
use rune::{
    runtime::{Ref, Vec as RuneVec},
    Any, FromValue, Module, ToValue, Value,
};
use std::{net::SocketAddr, rc::Rc, time::Duration};

create_wrapper!(ResolverWrapper, Resolver, Rc);

impl Clone for ResolverWrapper {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[rune::function]
fn create_system_resolver() -> Result<ResolverWrapper> {
    Ok(SystemResolver::default().into())
}

#[rune::function]
fn create_udp_resolver(addrs: RuneVec, timeout: u64) -> Result<ResolverWrapper> {
    Ok(HickoryResolver::new(
        addrs
            .into_iter()
            .map(|addr| {
                anyhow::Ok(NameServerConfig::new(
                    String::from_value(addr)?.parse::<SocketAddr>()?,
                    Protocol::Udp,
                ))
            })
            .try_collect()?,
        Duration::from_millis(timeout),
    )?
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
            .map(|ip| ip.to_string().to_value())
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
            .map(|ip| ip.to_string().to_value())
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
            .map(|ip| ip.to_string().to_value())
            .collect::<Result<Vec<Value>, _>>()?
            .try_into()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::engine::testing;

    use super::*;

    #[tokio::test]
    async fn test_create_system_resolver() -> Result<()> {
        let _: () = testing::run(
            vec![ResolverWrapper::module()?],
            r#"
                let resolver = create_system_resolver()?;

                ()
            "#,
        )
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_create_udp_resolver() -> Result<()> {
        let _: () = testing::run(
            vec![ResolverWrapper::module()?],
            r#"
                let resolver = create_udp_resolver([
                    "8.8.8.8:53",
                    "1.1.1.1:53"
                ], 5000)?;
            "#,
        )
        .await?;

        Ok(())
    }
}
