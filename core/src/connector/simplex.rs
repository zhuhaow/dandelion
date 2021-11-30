use super::Connector;
use crate::{io::Io, simplex::io::into_io, Endpoint, Result};
use tokio_tungstenite::{client_async, tungstenite::handshake::client::Request};

#[derive(thiserror::Error, Debug)]
pub enum SimplexError {
    #[error("Failed to create simplex connection header: {0}")]
    HeaderConfigInvalid(#[from] http::Error),
}

// Simplex is a lightweight protocol that based on WebSocket with only 1 extra RTT delay.
// I haven't implemented the server yet.

pub struct SimplexConnector<C: Connector> {
    next_hop: Endpoint,
    uri: String,
    secret_header: (String, String),
    connector: C,
}

impl<C: Connector> SimplexConnector<C> {
    pub fn new(
        next_hop: Endpoint,
        uri: String,
        secret_header: (String, String),
        connector: C,
    ) -> Self {
        Self {
            next_hop,
            uri,
            secret_header,
            connector,
        }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for SimplexConnector<C> {
    type Stream = Box<dyn Io>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let request: Request = Request::builder()
            .uri(&self.uri)
            .header(&self.secret_header.0, &self.secret_header.1)
            .header("Simplex Connect", endpoint.to_string())
            .body(())
            .map_err(Into::<SimplexError>::into)?;

        let next_hop_stream = self.connector.connect(&self.next_hop).await?;

        let (stream, _response) = client_async(request, next_hop_stream).await?;

        Ok(Box::new(into_io(stream)))
    }
}
