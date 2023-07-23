use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::Context;

pub async fn connect(endpoint: &Endpoint, nexthop: impl Io) -> Result<impl Io> {
    let s = tokio_native_tls::TlsConnector::from(
        tokio_native_tls::native_tls::TlsConnector::new()
            .context("Failed to create TLS connector")?,
    )
    .connect(&endpoint.hostname(), nexthop)
    .await
    .with_context(|| format!("Failed to establish a secure connection to {}", endpoint))?;

    Ok(s)
}
