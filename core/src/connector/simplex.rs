use super::Connector;
use crate::{Endpoint, Result};
use tokio_tungstenite::{client_async, tungstenite::handshake::client::Request};

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

impl<C: Connector> Connector for SimplexConnector<C> {
    type Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let request = Request::builder()
            .uri(self.uri)
            .header(self.secret_header.0, self.secret_header.1)
            .header("Simplex Connect", endpoint.to_string());

        let next_hop_stream = self.connector.connect(&self.next_hop).await?;

        let (stream, _response) = client_async(request, next_hop_stream).await?;

        Ok(stream)
    }
}
