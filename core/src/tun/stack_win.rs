use super::device::Device;
use crate::acceptor::Acceptor;
use crate::acceptor::NoOpAcceptor;
use crate::resolver::Resolver;
use crate::Result;
use futures::future::Ready;
use futures::Future;
use ipnetwork::Ipv4Network;
use std::net::SocketAddrV4;
use tokio::net::TcpStream;

pub async fn create_stack<R: Resolver>(
    _device: Device,
    _subnet: Ipv4Network,
    _resolver: R,
) -> Result<(impl Future<Output = ()>, impl Acceptor<TcpStream>)> {
    Err::<(Ready<()>, NoOpAcceptor), _>(anyhow::anyhow!("Not supported"))
}
