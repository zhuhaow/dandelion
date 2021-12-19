use super::{boxed::BoxedConnector, Connector, ConnectorFactory};
use crate::{endpoint::Endpoint, Result};
use anyhow::bail;

#[derive(Clone, Debug, Default)]
pub struct BlockConnector {}

#[async_trait::async_trait]
impl Connector for BlockConnector {
    type Stream = <BoxedConnector as Connector>::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        bail!("Connection to {} blocked", endpoint);
    }
}

pub struct BlockConnectorFactory {}

impl ConnectorFactory for BlockConnectorFactory {
    type Product = BlockConnector;

    fn build(&self) -> Self::Product {
        BlockConnector {}
    }
}
