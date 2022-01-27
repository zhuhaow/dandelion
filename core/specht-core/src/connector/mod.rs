pub mod block;
pub mod http;
pub mod pool;
pub mod rule;
pub mod simplex;
pub mod socks5;
pub mod speed;
pub mod tcp;
pub mod tls;

use crate::{endpoint::Endpoint, io::Io, Result};
use std::sync::Arc;

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

    fn arc(self) -> ArcConnector
    where
        Self: Sized + 'static,
    {
        Arc::new(ConnectorWrapper { connector: self })
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

pub type ArcConnector = Arc<dyn Connector<Stream = Box<dyn Io>>>;

#[async_trait::async_trait]
impl<C: Connector + ?Sized> Connector for Arc<C> {
    type Stream = C::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        C::connect(self, endpoint).await
    }
}
