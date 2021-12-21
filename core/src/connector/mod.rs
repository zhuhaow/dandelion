pub mod block;
pub mod http;
pub mod rule;
pub mod simplex;
pub mod socks5;
pub mod speed;
pub mod tcp;
pub mod tls;

use crate::{endpoint::Endpoint, io::Io, Result};
use dyn_clone::DynClone;

#[async_trait::async_trait]
pub trait Connector: Send + Sync + DynClone + 'static {
    type Stream: Io;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream>;

    fn boxed(self) -> BoxedConnector
    where
        Self: Sized + Clone,
    {
        Box::new(ConnectorWrapper { connector: self })
    }
}

dyn_clone::clone_trait_object!(Connector<Stream = Box<dyn Io>>);

#[derive(Clone)]
struct ConnectorWrapper<C: Connector + Clone> {
    connector: C,
}

#[async_trait::async_trait]
impl<C: Connector + Clone> Connector for ConnectorWrapper<C> {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let stream = self.connector.connect(endpoint).await?;

        Ok(Box::new(stream))
    }
}

pub type BoxedConnector = Box<dyn Connector<Stream = Box<dyn Io>>>;

#[async_trait::async_trait]
impl Connector for BoxedConnector {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        (*self).connect(endpoint).await
    }
}
