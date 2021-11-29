use crate::{Endpoint, Result};
use tokio::net::TcpStream;

// TODO: Implement RFC 8305

pub struct TcpConnector {}

impl TcpConnector {
    pub async fn connect(endpoint: &Endpoint) -> Result<TcpStream> {
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
