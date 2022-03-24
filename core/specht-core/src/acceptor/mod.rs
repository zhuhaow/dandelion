pub mod http;
pub mod quic;
pub mod simplex;
pub mod socks5;

use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::bail;
use futures::future::BoxFuture;

pub type HandshakeResult = Result<(Endpoint, BoxFuture<'static, Result<Box<dyn Io>>>)>;

#[as_dyn_trait::as_dyn_trait]
#[async_trait::async_trait]
pub trait Acceptor<I: Io>: Send + Sync {
    async fn do_handshake(&self, io: I) -> HandshakeResult;
}

pub struct NoOpAcceptor {}

#[async_trait::async_trait]
impl<I: Io> Acceptor<I> for NoOpAcceptor {
    async fn do_handshake(&self, _io: I) -> HandshakeResult {
        bail!("Not implemented")
    }
}
