pub mod boxed;
pub mod http;
pub mod rule;
pub mod simplex;
pub mod socks5;
pub mod speed;
pub mod tcp;
pub mod tls;

use crate::{endpoint::Endpoint, io::Io, Result};

#[async_trait::async_trait]
pub trait Connector: Send + Sync + 'static {
    type Stream: Io;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream>;
}

pub trait ConnectorFactory: Send + Sync + 'static {
    type Product: Connector;

    fn build(&self) -> Self::Product;
}
