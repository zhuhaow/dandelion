pub mod block;
pub mod http;
pub mod rule;
pub mod simplex;
pub mod socks5;
pub mod speed;
pub mod tcp;
pub mod tcp_pool;
pub mod tls;

use crate::{endpoint::Endpoint, io::Io, Result};

#[async_trait::async_trait]
pub trait Connector: Sync + Send {
    type Stream: Io;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream>;

    fn boxed(self) -> BoxedConnector
    where
        Self: Sized + 'static,
    {
        Box::new(ConnectorWrapper { connector: self })
    }
}

struct ConnectorWrapper<C: Connector> {
    connector: C,
}

#[async_trait::async_trait]
impl<C: Connector> Connector for ConnectorWrapper<C> {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        Ok(Box::new(self.connector.connect(endpoint).await?))
    }
}

pub type BoxedConnector = Box<dyn Connector<Stream = Box<dyn Io>>>;

#[async_trait::async_trait]
impl Connector for BoxedConnector {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        self.as_ref().connect(endpoint).await
    }
}
