pub mod system;

use crate::Result;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
    vec::IntoIter,
};

#[async_trait::async_trait]
pub trait Resolver: Sync + Send {
    async fn lookup_ip(&self, name: &str) -> Result<IntoIter<IpAddr>>;
    async fn lookup_ipv4(&self, name: &str) -> Result<IntoIter<Ipv4Addr>>;
    async fn lookup_ipv6(&self, name: &str) -> Result<IntoIter<Ipv6Addr>>;
}

#[async_trait::async_trait]
impl<R: Resolver + ?Sized> Resolver for Arc<R> {
    async fn lookup_ip(&self, name: &str) -> Result<IntoIter<IpAddr>> {
        R::lookup_ip(self, name).await
    }

    async fn lookup_ipv4(&self, name: &str) -> Result<IntoIter<Ipv4Addr>> {
        R::lookup_ipv4(self, name).await
    }

    async fn lookup_ipv6(&self, name: &str) -> Result<IntoIter<Ipv6Addr>> {
        R::lookup_ipv6(self, name).await
    }
}
