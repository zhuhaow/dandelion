use super::{
    boxed::{BoxedConnector, BoxedConnectorFactory},
    Connector, ConnectorFactory,
};
use crate::endpoint::Endpoint;
use anyhow::Result;
use futures::future::{select_ok, FutureExt};
use std::time::Duration;
use tokio::time::sleep;

pub struct SpeedConnector {
    connectors: Vec<(Duration, BoxedConnector)>,
}

impl SpeedConnector {
    pub fn new(connectors: Vec<(Duration, BoxedConnector)>) -> Self {
        Self { connectors }
    }
}

#[async_trait::async_trait]
impl Connector for SpeedConnector {
    type Stream = <BoxedConnector as Connector>::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        select_ok(self.connectors.iter().map(|c| {
            async move {
                sleep(c.0).await;

                c.1.connect(endpoint).await
            }
            .boxed()
        }))
        .await
        .map(|r| r.0)
    }
}

pub struct SpeedConnectorFactory {
    factories: Vec<(Duration, BoxedConnectorFactory)>,
}

impl SpeedConnectorFactory {
    pub fn new(factories: Vec<(Duration, BoxedConnectorFactory)>) -> Self {
        Self { factories }
    }
}

impl ConnectorFactory for SpeedConnectorFactory {
    type Product = SpeedConnector;

    fn build(&self) -> Self::Product {
        SpeedConnector::new(self.factories.iter().map(|f| (f.0, f.1.build())).collect())
    }
}
