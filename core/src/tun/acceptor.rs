use super::{dns::FakeDns, translator::Translator};

use crate::{
    acceptor::{Acceptor, HandshakeResult},
    endpoint::Endpoint,
    io::Io,
};
use anyhow::bail;
use async_trait::async_trait;
use futures::FutureExt;
use std::sync::Arc;
use tokio::{net::TcpStream, sync::Mutex};

pub struct TunAcceptor {
    dns_server: Arc<FakeDns>,
    translator: Arc<Mutex<Translator>>,
}

impl TunAcceptor {
    pub fn new(dns_server: Arc<FakeDns>, translator: Arc<Mutex<Translator>>) -> Self {
        Self {
            dns_server,
            translator,
        }
    }
}

#[async_trait]
impl Acceptor<TcpStream> for TunAcceptor {
    async fn do_handshake(&self, io: TcpStream) -> HandshakeResult {
        let remote_addr = io.peer_addr()?;
        let addr = match remote_addr {
            std::net::SocketAddr::V4(addr) => addr,
            std::net::SocketAddr::V6(_) => bail!("Do not support Ipv6 for tun yet"),
        };
        let fake_addr = self
            .translator
            .lock()
            .await
            .look_up_source(&addr)
            .ok_or_else(|| anyhow::anyhow!("Failed to find SNAT address: {}", addr))?;

        let domain = self
            .dns_server
            .reverse_lookup(fake_addr.ip())
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find DNAT address: {}", fake_addr))?;

        let io: Box<dyn Io> = Box::new(io);

        Ok((
            Endpoint::new_from_domain(&domain, fake_addr.port()),
            futures::future::ready(Ok(io)).boxed(),
        ))
    }
}
