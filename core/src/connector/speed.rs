use super::Connector;
use crate::endpoint::Endpoint;
use anyhow::Result;
use futures::future::{select_ok, FutureExt};
use std::time::Duration;
use tokio::time::sleep;

pub struct SpeedConnector<C: Connector> {
    connectors: Vec<(Duration, C)>,
}

impl<C: Connector> SpeedConnector<C> {
    pub fn new(connectors: Vec<(Duration, C)>) -> Self {
        Self { connectors }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for SpeedConnector<C> {
    type Stream = C::Stream;

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
