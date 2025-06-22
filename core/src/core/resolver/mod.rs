pub mod hickory;
pub mod system;

use crate::Result;
use anyhow::bail;
use hickory_proto::op::Message;
use std::{
    fmt::Debug,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    vec::Vec,
};

#[async_trait::async_trait]
#[auto_impl::auto_impl(Rc)]
pub trait Resolver: Debug {
    async fn lookup_ip(&self, name: &str) -> Result<Vec<IpAddr>>;
    async fn lookup_ipv4(&self, name: &str) -> Result<Vec<Ipv4Addr>>;
    async fn lookup_ipv6(&self, name: &str) -> Result<Vec<Ipv6Addr>>;
    async fn lookup_raw(&self, _message: Message) -> Result<Message> {
        bail!("Not implemented")
    }
}
