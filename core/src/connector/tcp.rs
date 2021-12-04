use super::Connector;
use crate::{endpoint::Endpoint, Result};
use tokio::net::TcpStream;

// TODO: Implement RFC 8305

#[derive(Clone, Debug, Default)]
pub struct TcpConnector {}

#[async_trait::async_trait]
impl Connector for TcpConnector {
    type Stream = TcpStream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let addr = match endpoint {
            Endpoint::Addr(addr) => addr.to_owned(),
            Endpoint::Domain(host, port) => tokio::net::lookup_host((host.as_str(), *port))
                .await?
                .next()
                .ok_or(crate::resolver::ResolverError::NoEntry)?,
        };

        let stream = TcpStream::connect(addr).await?;
        Ok(stream)
    }
}
