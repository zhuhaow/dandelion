use super::Connector;
use crate::{endpoint::Endpoint, Result};
use anyhow::{ensure, Context};
use http::{Request, StatusCode};
use hyper::{client::conn::handshake, Body};
use tracing::debug;

pub struct HttpConnector<C: Connector> {
    connector: C,
    next_hop: Endpoint,
}

impl<C: Connector> HttpConnector<C> {
    pub fn new(connector: C, next_hop: Endpoint) -> Self {
        Self {
            connector,
            next_hop,
        }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for HttpConnector<C> {
    type Stream = C::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        debug!("Begin HTTP CONNECT handshake");

        let s = self
            .connector
            .connect(&self.next_hop)
            .await
            .with_context(|| format!("Failed to connect to next hop {}", &self.next_hop))?;

        let (mut request_sender, connection) = handshake(s)
            .await
            .with_context(|| "Failed to do hyper client handshake")?;

        let request = Request::builder()
            .method("CONNECT")
            .uri(endpoint.to_string())
            .body(Body::from(""))
            .with_context(|| format!("Failed to create CONNECT request to {}", endpoint))?;

        let mut response_future = request_sender.send_request(request);

        let mut connection = connection.without_shutdown();

        let (maybe_parts, response) = tokio::select! {
            parts = &mut connection => {
                // We don't really care the result here. We will read the result from the other future.
                (Some(parts), response_future.await)
            },
            result = &mut response_future => (None, result),
        };

        let response = response.with_context(|| "Failed to send CONNECT request")?;

        ensure!(
            response.status() == StatusCode::OK,
            "CONNECT failed with response code {}",
            response.status()
        );

        let parts = match maybe_parts {
            Some(p) => p,
            None => connection.await,
        }
        .with_context(|| "Failed to obtain the underlying io after handshake")?;

        debug!("Finished HTTP CONNECT handshake");

        Ok(parts.io)
    }
}
