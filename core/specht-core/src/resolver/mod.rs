pub mod system;
pub mod trust;

use crate::Result;
use anyhow::bail;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
    vec::Vec,
};
use trust_dns_proto::op::Message;

#[async_trait::async_trait]
pub trait Resolver: Sync + Send {
    async fn lookup_ip(&self, name: &str) -> Result<Vec<IpAddr>>;
    async fn lookup_ipv4(&self, name: &str) -> Result<Vec<Ipv4Addr>>;
    async fn lookup_ipv6(&self, name: &str) -> Result<Vec<Ipv6Addr>>;
    async fn lookup_raw(&self, _message: Message) -> Result<Message> {
        bail!("Not implemented")
    }
}

#[async_trait::async_trait]
impl<R: Resolver + ?Sized> Resolver for Arc<R> {
    async fn lookup_ip(&self, name: &str) -> Result<Vec<IpAddr>> {
        R::lookup_ip(self, name).await
    }

    async fn lookup_ipv4(&self, name: &str) -> Result<Vec<Ipv4Addr>> {
        R::lookup_ipv4(self, name).await
    }

    async fn lookup_ipv6(&self, name: &str) -> Result<Vec<Ipv6Addr>> {
        R::lookup_ipv6(self, name).await
    }

    async fn lookup_raw(&self, message: Message) -> Result<Message> {
        R::lookup_raw(self, message).await
    }
}
