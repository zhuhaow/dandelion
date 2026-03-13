use crate::{
    config::engine::resolver::ResolverWrapper,
    core::{resolver::Resolver, tun::resolver::FakeDnsResolver},
    Result,
};
use rune::{
    alloc::clone::TryClone,
    runtime::{Object, RuntimeContext},
    Any, Unit, Vm,
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Any)]
pub struct FakeResolver {
    inner: Rc<Mutex<FakeDnsResolver>>,
}

impl FakeResolver {
    pub fn new(inner: Rc<Mutex<FakeDnsResolver>>) -> Self {
        Self { inner }
    }

    #[rune::function]
    pub fn lookup_ipv4(&self, name: &str) -> Result<String> {
        self.inner
            .lock()
            .unwrap()
            .lookup_ipv4(name)
            .map(|ip| ip.to_string())
            .ok_or_else(|| anyhow::anyhow!("No fake IPv4 assigned for {}, possibly due to fake DNS doesn't have an IPv4 pool.", name))
    }

    #[rune::function]
    pub fn lookup_ipv6(&self, name: &str) -> Result<String> {
        self.inner


            .lock()
            .unwrap()
            .lookup_ipv6(name)
            .map(|ip| ip.to_string())
            .ok_or_else(|| anyhow::anyhow!("No fake IPv6 assigned for {}, possibly due to fake DNS doesn't have an IPv6 pool.", name))
    }
}

#[allow(dead_code)]
pub struct DnsServer<R: Resolver> {
    // Used for handle non A/AAAA queries, such as TXT, CNAME, PTR (should we handle PTR?) etc.
    fallback_server: R,
    resolver: FakeResolver,
    dns_handler: String,
    context: Arc<RuntimeContext>,
    unit: Arc<Unit>,
    cache: Object,
}

#[derive(Any)]
pub enum ResolveStrategy {
    Resolver(#[rune(get, set)] ResolverWrapper),
    Ip(#[rune(get, set)] String),
    NxDomain,
}

#[allow(dead_code)]
impl<R: Resolver> DnsServer<R> {
    pub fn new(
        fallback_server: R,
        resolver: FakeResolver,
        dns_handler: String,
        context: Arc<RuntimeContext>,
        unit: Arc<Unit>,
        cache: Object,
    ) -> Self {
        Self {
            fallback_server,
            resolver,
            dns_handler,
            context,
            unit,
            cache,
        }
    }

    pub async fn handle_message(&self, domain: &str) -> Result<ResolveStrategy> {
        let mut vm = Vm::new(self.context.clone(), self.unit.clone());

        Ok(rune::from_value(
            vm.async_call(
                [self.dns_handler.as_str()],
                (domain, self.cache.try_clone()?),
            )
            .await?,
        )?)
    }
}
