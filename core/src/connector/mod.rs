pub mod simplex;
pub mod tcp;
pub mod tls;

use crate::{endpoint::Endpoint, io::Io, Result};

#[async_trait::async_trait]
pub trait Connector: Sync + Send + 'static {
    type Stream: Io;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream>;
}

pub trait ConnectorFactory {
    type Product: Connector;

    fn build(&self) -> Self::Product;
}

struct ConnectorWrapper<C: Connector> {
    connector: C,
}

#[async_trait::async_trait]
impl<C: Connector> Connector for ConnectorWrapper<C> {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let stream = self.connector.connect(endpoint).await?;

        Ok(Box::new(stream))
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for Box<C> {
    type Stream = <C as Connector>::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let stream = self.as_ref().connect(endpoint).await?;
        Ok(stream)
    }
}

pub struct BoxedConnector {
    connector: Box<dyn Connector<Stream = Box<dyn Io>>>,
}

impl BoxedConnector {
    pub fn new<I: Io, C: Connector<Stream = I>>(connector: C) -> Self {
        Self {
            connector: Box::new(ConnectorWrapper {
                connector: Box::new(connector),
            }),
        }
    }
}

#[async_trait::async_trait]
impl Connector for BoxedConnector {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let stream = self.connector.connect(endpoint).await?;
        Ok(stream)
    }
}
