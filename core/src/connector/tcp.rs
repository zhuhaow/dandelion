use crate::{Endpoint, Result};
use tokio::net::TcpStream;

use super::Connector;

// TODO: Implement RFC 8305

pub struct TcpConnector {}

#[async_trait::async_trait]
impl Connector<TcpStream> for TcpConnector {
    async fn connect(self, endpoint: &Endpoint) -> Result<TcpStream> {
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
