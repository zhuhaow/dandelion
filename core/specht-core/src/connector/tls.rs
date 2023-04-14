use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::Context;
use futures::Future;
use tokio_native_tls::TlsStream;

pub async fn connect<I: Io, F: Future<Output = Result<I>>, C: Fn(&Endpoint) -> F>(
    connector: C,
    endpoint: &Endpoint,
) -> Result<TlsStream<I>> {
    let s = connector(endpoint)
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
