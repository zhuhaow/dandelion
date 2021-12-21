use super::Connector;
use crate::{endpoint::Endpoint, Result};
use tokio::net::TcpStream;

#[derive(Debug, Default)]
pub struct TcpConnector;

#[async_trait::async_trait]
impl Connector for TcpConnector {
    type Stream = TcpStream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        // TODO: Implement RFC 8305
        let addr = match endpoint {
            Endpoint::Addr(addr) => addr.to_owned(),
            Endpoint::Domain(host, port) => tokio::net::lookup_host((host.as_str(), *port))
                .await?
                .next()
                .ok_or_else(|| {
                    anyhow::anyhow!("Endpoint {} is resolved with no records", endpoint)
                })?,
        };

        let stream = TcpStream::connect(addr).await?;
        Ok(stream)
    }
}
