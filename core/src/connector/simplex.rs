use super::Connector;
use crate::{io::Io, Endpoint, Result};
use async_compat::Compat;
use futures::{
    stream::{IntoAsyncRead, TryStreamExt},
    SinkExt, Stream,
};
use std::task::Poll;
use tokio_tungstenite::{
    client_async,
    tungstenite::{error::Error as WsError, handshake::client::Request, Message},
    WebSocketStream,
};

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
    type Stream = Compat<IntoAsyncRead<WebSocketStreamToAsyncWrite<C::Stream>>>;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let request: Request = Request::builder()
            .uri(&self.uri)
            .header(&self.secret_header.0, &self.secret_header.1)
            .header("Simplex Connect", endpoint.to_string())
            .body(())
            .map_err(Into::<SimplexError>::into)?;

        let next_hop_stream = self.connector.connect(&self.next_hop).await?;

        let (stream, _response) = client_async(request, next_hop_stream).await?;

        let stream = WebSocketStreamToAsyncWrite { stream }.into_async_read();

        Ok(Compat::new(stream))
    }
}

#[pin_project::pin_project]
pub struct WebSocketStreamToAsyncWrite<C: Io> {
    stream: WebSocketStream<C>,
}

fn ws_to_io_error(error: WsError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, error)
}

// Here we implement futures AsyncWrite and then use Compat to support tokio's.
impl<C: Io> futures::io::AsyncWrite for WebSocketStreamToAsyncWrite<C> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let stream = self.project().stream;

        let result = futures::ready!(stream.poll_ready_unpin(cx));

        // TODO: There could be a better way to handle this, we are making copies.
        std::task::Poll::Ready(
            result
                .and_then(|_| {
                    stream
                        .start_send_unpin(Message::Binary(buf.to_owned()))
                        .map(|_| buf.len())
                })
                .map_err(ws_to_io_error),
        )
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let stream = self.project().stream;

        let result = futures::ready!(stream.poll_flush_unpin(cx));

        Poll::Ready(result.map_err(ws_to_io_error))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let stream = self.project().stream;

        let result = futures::ready!(stream.poll_flush_unpin(cx));

        Poll::Ready(result.map_err(ws_to_io_error))
    }
}

pub struct MessageWrapper(Message);

static EMPTY_BUF: [u8; 0] = [];

impl AsRef<[u8]> for MessageWrapper {
    fn as_ref(&self) -> &[u8] {
        match self.0 {
            Message::Binary(ref bytes) => bytes.as_ref(),
            _ => EMPTY_BUF.as_ref(),
        }
    }
}

impl<C: Io> Stream for WebSocketStreamToAsyncWrite<C> {
    type Item = std::io::Result<MessageWrapper>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let stream = self.project().stream;
        stream
            .try_poll_next_unpin(cx)
            .map_ok(MessageWrapper)
            .map_err(ws_to_io_error)
    }
}
