pub mod http;
pub mod simplex;
pub mod socks5;

use crate::{endpoint::Endpoint, io::Io, Result};
use futures::future::BoxFuture;

pub type HandshakeResult = Result<(Endpoint, BoxFuture<'static, Result<Box<dyn Io>>>)>;

#[async_trait::async_trait]
pub trait Acceptor<I: Io>: Send + Sync {
    async fn do_handshake(&self, io: I) -> HandshakeResult;
}
