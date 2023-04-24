use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::Context;
use futures::Future;

pub async fn connect<F: Future<Output = Result<impl Io>>, C: FnOnce(&Endpoint) -> F>(
    connector: C,
    endpoint: &Endpoint,
) -> Result<impl Io> {
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
