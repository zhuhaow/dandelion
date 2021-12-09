use super::Connector;
use crate::{endpoint::Endpoint, Result};
use tokio_native_tls::TlsStream;

#[derive(Clone, Debug)]
pub struct TlsConector<C: Connector> {
    connector: C,
}

impl<C: Connector> TlsConector<C> {
    pub fn new(connector: C) -> Self {
        Self { connector }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for TlsConector<C> {
    type Stream = TlsStream<C::Stream>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let s = self.connector.connect(endpoint).await?;
        let s = tokio_native_tls::TlsConnector::from(
            tokio_native_tls::native_tls::TlsConnector::new()?,
        )
        .connect(&endpoint.hostname(), s)
        .await?;
        Ok(s)
    }
}