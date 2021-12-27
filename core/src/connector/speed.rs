use super::{BoxedConnector, Connector};
use crate::{endpoint::Endpoint, io::Io};
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
    type Stream = Box<dyn Io>;

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