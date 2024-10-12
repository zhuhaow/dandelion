use super::{io::into_io, Config, ENDPOINT_HEADER_KEY};
use crate::{
    core::{endpoint::Endpoint, io::Io},
    Result,
};
use anyhow::{anyhow, bail, ensure, Context};
use bytes::{Buf, Bytes};
use chrono::Utc;
use futures::{Future, FutureExt};
use http_body_util::Full;
use hyper::{body::Incoming, server::conn::http1::Builder, service::service_fn, Request, Response};
use hyper_tungstenite::{
    is_upgrade_request,
    tungstenite::{error::ProtocolError, handshake::derive_accept_key, protocol::Role},
    WebSocketStream,
};
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{
        oneshot::{channel, Receiver, Sender},
        Mutex,
    },
};
use tracing::info;

async fn hide_error_handler(
    request: Request<Incoming>,
    config: Config,
    signal: Arc<Mutex<Option<UpgradeSignal>>>,
) -> Result<Response<Full<Bytes>>> {
    let result = handler(request, config, signal).await;

    match result {
        Ok(response) => Ok(response),
        Err(err) => {
            info!(
                "Failed to process incoming simplex request. \
                   It's most likely the client is not a \
                   valid simplex client or the configuration is wrong. \
                   Hiding the error for security reasons. Error: {}",
                err
            );

            Ok(Response::new(Full::new(Bytes::from(format!(
                "Now is {}",
                Utc::now().to_rfc3339()
            )))))
        }
    }
}

async fn handler(
    request: Request<Incoming>,
    config: Config,
    signal: Arc<Mutex<Option<UpgradeSignal>>>,
) -> Result<Response<Full<Bytes>>> {
    // Check if the request is requesting the right path
    ensure!(
        request.uri().path() == config.path,
        "Got a simplex request to wrong path: {}",
        request.uri().path()
    );

    ensure!(
        request
            .headers()
            .get(config.secret_header.0.as_str())
            .and_then(|v| v.to_str().ok())
            == Some(config.secret_header.1.as_str()),
        "Got a simplex request with wrong secret header value."
    );

    ensure!(
        is_upgrade_request(&request),
        "Got a non upgrade request when simplex request is expected"
    );

    let endpoint = request
        .headers()
        .get(ENDPOINT_HEADER_KEY)
        .and_then(|ep| ep.to_str().ok())
        .and_then(|ep| ep.parse().ok())
        .ok_or_else(|| anyhow!("Failed to find valid target endpoint from simplex request"))?;

    let upgrade_signal = signal
        .lock()
        .await
        .take()
        .expect("there should be only one upgrade request for one connection");

    upgrade_signal
        .endpoint_tx
        .send(endpoint)
        .expect("the other side should not be released");

    upgrade_signal
        .done_rx
        .await
        .expect("the done signal should be sent before polling the connection");

    let response =
        upgrade_response(&request).context("Failed to create websocket upgrade response")?;

    Ok(response)
}

// From hyper_tungstenite
fn upgrade_response(request: &Request<Incoming>) -> Result<Response<Full<Bytes>>> {
    let key = request
        .headers()
        .get("Sec-WebSocket-Key")
        .ok_or(ProtocolError::MissingSecWebSocketKey)?;

    if request
        .headers()
        .get("Sec-WebSocket-Version")
        .map(|v| v.as_bytes())
        != Some(b"13")
    {
        return Err(ProtocolError::MissingSecWebSocketVersionHeader.into());
    }

    Ok(Response::builder()
        .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
        .header(hyper::header::CONNECTION, "upgrade")
        .header(hyper::header::UPGRADE, "websocket")
        .header("Sec-WebSocket-Accept", &derive_accept_key(key.as_bytes()))
        .body(Full::new(Bytes::from("switching to websocket protocol")))
        .expect("bug: failed to build response"))
}

struct UpgradeSignal {
    endpoint_tx: Sender<Endpoint>,
    done_rx: Receiver<()>,
}

pub async fn handshake(
    io: impl Io,
    config: Config,
) -> Result<(Endpoint, impl Future<Output = Result<impl Io>>)> {
    let (done_tx, done_rx) = channel();
    let (endpoint_tx, endpoint_rx) = channel();

    let signal = Arc::new(Mutex::new(Some(UpgradeSignal {
        endpoint_tx,
        done_rx,
    })));

    let conn = Builder::new().serve_connection(
        TokioIo::new(io),
        service_fn(move |req| {
            let config = config.clone();
            let signal = signal.clone();
            // We need to pin the future here so the `conn` is `Unpin`able.
            hide_error_handler(req, config, signal).boxed()
        }),
    );

    let mut conn_fut = conn.without_shutdown();

    let endpoint = tokio::select! {
        _ = &mut conn_fut => {
            // No upgrade happens. The client isn't a
            // simplex client;
            bail!("The client is not a valid simplex client");
        }
        result = endpoint_rx => {
            match result {
                Ok(endpoint) => endpoint,
                Err(_) => unreachable!(),
            }
        }
    };

    info!("Got connection request to {}", endpoint);

    Ok((endpoint, async move {
        // This should never error since we are not polling the other side, so
        // the receiver should not be deallocated.
        done_tx
            .send(())
            .expect("bug: the done signal receiver should not be deallocated");
        let part = conn_fut.await?;

        let ws_stream = WebSocketStream::from_raw_socket(
            ChainReadBufAndIo {
                read_buf: part.read_buf,
                io: part.io.into_inner(),
            },
            Role::Server,
            None,
        )
        .await;

        Ok(into_io(ws_stream))
    }))
}

#[derive(Debug)]
#[pin_project::pin_project]
struct ChainReadBufAndIo<I: Io> {
    // TODO: Make this Option<>
    read_buf: Bytes,
    #[pin]
    io: I,
}

impl<I: Io> AsyncRead for ChainReadBufAndIo<I> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.project();

        if !this.read_buf.is_empty() {
            let len = this.read_buf.len().min(buf.remaining());
            buf.put_slice(&this.read_buf.slice(0..len));
            this.read_buf.advance(len);
            return std::task::Poll::Ready(Ok(()));
        }

        this.io.poll_read(cx, buf)
    }
}

impl<I: Io> AsyncWrite for ChainReadBufAndIo<I> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.project().io.poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().io.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().io.poll_shutdown(cx)
    }
}
