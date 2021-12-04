pub mod simplex;
pub mod socks5;

use crate::{endpoint::Endpoint, io::Io, Result};

#[async_trait::async_trait]
pub trait Acceptor: Clone + Send + 'static {
    type Input: Io;
    type Output: MidHandshake;

    async fn handshake(self, io: Self::Input) -> Result<Self::Output>;
}

#[async_trait::async_trait]
pub trait MidHandshake: Send {
    type Output: Io;

    fn target_endpoint(&self) -> &Endpoint;
    async fn finalize(self) -> Result<Self::Output>;
}
