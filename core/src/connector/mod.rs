pub mod simplex;
pub mod tcp;

use crate::{io::Io, Endpoint, Result};

#[async_trait::async_trait]
pub trait Connector: Sync + Send + 'static {
    type Stream: Io;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream>;
}

struct _ConnectorWrapper<Conn: Connector> {
    connector: Conn,
}

#[async_trait::async_trait]
impl<Conn: Connector> Connector for _ConnectorWrapper<Conn> {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let stream = self.connector.connect(endpoint).await?;
        Ok(Box::new(stream))
    }
}

pub struct ConnectorWrapper {
    connector: Box<dyn Connector<Stream = Box<dyn Io>>>,
}

impl ConnectorWrapper {
    pub fn new<Conn: Connector>(connector: Conn) -> Self {
        Self {
            connector: Box::new(_ConnectorWrapper { connector }),
        }
    }
}

#[async_trait::async_trait]
impl Connector for ConnectorWrapper {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        Ok(self.connector.connect(endpoint).await?)
    }
}

pub trait ConnectorFactory: 'static {
    type Product: Connector;

    fn build(&self) -> Self::Product;
}

struct _ConnectorFactoryWrapper<Factory: ConnectorFactory> {
    factory: Factory,
}

impl<Factory: ConnectorFactory> ConnectorFactory for _ConnectorFactoryWrapper<Factory> {
    type Product = ConnectorWrapper;

    fn build(&self) -> Self::Product {
        ConnectorWrapper::new(self.factory.build())
    }
}

pub struct ConnectorFactoryWrapper {
    factory: Box<dyn ConnectorFactory<Product = ConnectorWrapper>>,
}

impl ConnectorFactoryWrapper {
    pub fn new<Factory: ConnectorFactory>(factory: Factory) -> Self {
        Self {
            factory: Box::new(_ConnectorFactoryWrapper { factory }),
        }
    }
}

impl ConnectorFactory for ConnectorFactoryWrapper {
    type Product = ConnectorWrapper;

    fn build(&self) -> Self::Product {
        self.factory.build()
    }
}
