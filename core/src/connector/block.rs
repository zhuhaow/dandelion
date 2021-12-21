use super::Connector;
use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::bail;

#[derive(Clone, Debug, Default)]
pub struct BlockConnector {}

#[async_trait::async_trait]
impl Connector for BlockConnector {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        bail!("Connection to {} blocked", endpoint);
    }
}
