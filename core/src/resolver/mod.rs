pub mod system;

use crate::Result;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    vec::IntoIter,
};

#[async_trait::async_trait]
pub trait Resolver {
    async fn lookup_ip(&self, name: &str) -> Result<IntoIter<IpAddr>>;
    async fn lookup_ipv4(&self, name: &str) -> Result<IntoIter<Ipv4Addr>>;
    async fn lookup_ipv6(&self, name: &str) -> Result<IntoIter<Ipv6Addr>>;
}
