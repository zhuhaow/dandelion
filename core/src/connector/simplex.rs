use super::{Connector, ConnectorFactory};
use crate::{
    endpoint::Endpoint,
    io::Io,
    simplex::{client::connect, Config},
    Result,
};

pub struct SimplexConnector<C: Connector> {
    next_hop: Endpoint,
    config: Config,
    connector: C,
}

impl<C: Connector> SimplexConnector<C> {
    pub fn new(next_hop: Endpoint, config: Config, connector: C) -> Self {
        Self {
            next_hop,
            config,
            connector,
        }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for SimplexConnector<C> {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let s = connect(
            self.connector.connect(&self.next_hop).await?,
            endpoint,
            &self.config,
            self.next_hop.to_string(),
        )
        .await?;

        Ok(Box::new(s))
    }
}

pub struct SimplexConnectorFactory<F: ConnectorFactory> {
    factory: F,
    next_hop: Endpoint,
    config: Config,
}

impl<F: ConnectorFactory> SimplexConnectorFactory<F> {
    pub fn new(factory: F, next_hop: Endpoint, config: Config) -> Self {
        Self {
            factory,
            next_hop,
            config,
        }
    }
}

impl<F: ConnectorFactory> ConnectorFactory for SimplexConnectorFactory<F> {
    type Product = SimplexConnector<F::Product>;

    fn build(&self) -> Self::Product {
        SimplexConnector::new(
            self.next_hop.clone(),
            self.config.clone(),
            self.factory.build(),
        )
    }
}
