use super::device::Device;
use crate::{
    acceptor::{Acceptor, NoOpAcceptor},
    resolver::Resolver,
    Result,
};
use futures::{future::Ready, Future};
use ipnetwork::Ipv4Network;
use tokio::net::TcpStream;

pub async fn create_stack<R: Resolver>(
    _device: Device,
    _subnet: Ipv4Network,
    _resolver: R,
) -> Result<(impl Future<Output = ()>, impl Acceptor<TcpStream>)> {
    Err::<(Ready<()>, NoOpAcceptor), _>(anyhow::anyhow!("Not supported"))
}
