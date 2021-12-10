use super::{Connector, ConnectorFactory};
use crate::{endpoint::Endpoint, io::Io, Result};

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

pub struct BoxedConnector {
    connector: Box<dyn Connector<Stream = Box<dyn Io>>>,
}

impl BoxedConnector {
    pub fn new<I: Io, C: Connector<Stream = I>>(connector: C) -> Self {
        Self {
            connector: Box::new(ConnectorWrapper { connector }),
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

struct ConnectorFactoryWrapper<F: ConnectorFactory> {
    factory: F,
}

impl<F: ConnectorFactory> ConnectorFactory for ConnectorFactoryWrapper<F> {
    type Product = BoxedConnector;

    fn build(&self) -> Self::Product {
        BoxedConnector::new(self.factory.build())
    }
}

pub struct BoxedConnectorFactory {
    factory: Box<dyn ConnectorFactory<Product = BoxedConnector>>,
}

impl BoxedConnectorFactory {
    pub fn new<F: ConnectorFactory>(factory: F) -> Self {
        Self {
            factory: Box::new(ConnectorFactoryWrapper { factory }),
        }
    }
}

impl ConnectorFactory for BoxedConnectorFactory {
    type Product = BoxedConnector;

    fn build(&self) -> Self::Product {
        self.factory.build()
    }
}
