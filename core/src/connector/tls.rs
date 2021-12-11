use super::{Connector, ConnectorFactory};
use crate::{endpoint::Endpoint, Result};
use anyhow::Context;
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
        let s = self
            .connector
            .connect(endpoint)
            .await
            .with_context(|| format!("Failed to connect to the next hop {}", endpoint))?;

        let s = tokio_native_tls::TlsConnector::from(
            tokio_native_tls::native_tls::TlsConnector::new()
                .context("Failed to create TLS connector")?,
        )
        .connect(&endpoint.hostname(), s)
        .await
        .with_context(|| format!("Failed to establish a secure connection to {}", endpoint))?;

        Ok(s)
    }
}

pub struct TlsConnectorFactory<F: ConnectorFactory> {
    factory: F,
}

impl<F: ConnectorFactory> TlsConnectorFactory<F> {
    pub fn new(factory: F) -> Self {
        Self { factory }
    }
}

impl<F: ConnectorFactory> ConnectorFactory for TlsConnectorFactory<F> {
    type Product = TlsConector<F::Product>;

    fn build(&self) -> Self::Product {
        TlsConector::new(self.factory.build())
    }
}
