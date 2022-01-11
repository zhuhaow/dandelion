use crate::tun::device::Device;
use crate::Result;
use anyhow::bail;
use ipnetwork::Ipv4Network;
use std::net::SocketAddr;

// Delegate the actions that we cannot do with normal permission to external
// services.
#[async_trait::async_trait]
pub trait PrivilegeHandler {
    async fn set_http_proxy(&self, addr: Option<SocketAddr>) -> Result<()>;
    async fn set_socks5_proxy(&self, addr: Option<SocketAddr>) -> Result<()>;
    async fn create_tun_interface(&self, subnet: &Ipv4Network) -> Result<Device>;
    async fn set_dns(&self, addr: Option<SocketAddr>) -> Result<()>;
}

#[derive(Default)]
pub struct NoPrivilegeHandler {}

#[async_trait::async_trait]
impl PrivilegeHandler for NoPrivilegeHandler {
    async fn set_http_proxy(&self, addr: Option<SocketAddr>) -> Result<()> {
        bail!("No permission");
    }

    async fn set_socks5_proxy(&self, addr: Option<SocketAddr>) -> Result<()> {
        bail!("No permission");
    }

    async fn create_tun_interface(&self, subnet: &Ipv4Network) -> Result<Device> {
        bail!("No permission");
    }

    async fn set_dns(&self, addr: Option<SocketAddr>) -> Result<()> {
        bail!("No permission");
    }
}
