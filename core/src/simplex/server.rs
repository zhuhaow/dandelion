use super::{Config, ENDPOINT_HEADER_KEY};
use crate::{endpoint::Endpoint, io::Io, simplex::SimplexError, Result};
use bytes::{Buf, Bytes};
use chrono::Utc;
use futures::{future::BoxFuture, FutureExt, TryFutureExt};
use hyper::{server::conn::Http, service::service_fn, Body, Request, Response};
use hyper_tungstenite::{
    is_upgrade_request,
    tungstenite::{error::ProtocolError, handshake::derive_accept_key},
};
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{
        oneshot::{channel, Receiver, Sender},
        Mutex,
    },
};

fn create_empty_response() -> Response<Body> {
    Response::new(Body::from(format!("Now is {}", Utc::now().to_rfc3339())))
}

async fn handler(
    request: Request<Body>,
    config: Config,
    signal: Arc<Mutex<Option<UpgradeSignal>>>,
) -> Result<Response<Body>> {
    // Check if the request is requesting the right path
    if request.uri().path() != config.path {
        return Ok(create_empty_response());
    }

    if request
        .headers()
        .get(config.secret_header.0.as_str())
        .and_then(|v| v.to_str().ok())
        != Some(config.secret_header.1.as_str())
    {
        return Ok(create_empty_response());
    }

    if is_upgrade_request(&request) {
        let response = match upgrade_response(&request) {
            Ok(r) => r,
            Err(_) => return Ok(create_empty_response()),
        };

        let endpoint = match request
            .headers()
            .get(ENDPOINT_HEADER_KEY)
            .and_then(|ep| ep.to_str().ok())
            .and_then(|ep| ep.parse().ok())
        {
            Some(ep) => ep,
            None => return Ok(create_empty_response()),
        };

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

        Ok(response)
    } else {
        Ok(create_empty_response())
    }
}

// From hyper_tungstenite
fn upgrade_response(request: &Request<Body>) -> Result<Response<Body>> {
    let key =
        request
            .headers()
            .get("Sec-WebSocket-Key")
            .ok_or(tungstenite::error::Error::Protocol(
                ProtocolError::MissingSecWebSocketKey,
            ))?;
    if request
        .headers()
        .get("Sec-WebSocket-Version")
        .map(|v| v.as_bytes())
        != Some(b"13")
    {
        return Err(tungstenite::error::Error::Protocol(
            ProtocolError::MissingSecWebSocketVersionHeader,
        )
        .into());
    }

    Ok(Response::builder()
        .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
        .header(hyper::header::CONNECTION, "upgrade")
        .header(hyper::header::UPGRADE, "websocket")
        .header("Sec-WebSocket-Accept", &derive_accept_key(key.as_bytes()))
        .body(Body::from("switching to websocket protocol"))
        .expect("bug: failed to build response"))
}

pub struct SimplexMidHandshake {
    done: Sender<()>,
    endpoint: Endpoint,
    f: BoxFuture<'static, Result<Box<dyn Io>>>,
}

impl SimplexMidHandshake {
    fn new(
        done: Sender<()>,
        endpoint: Endpoint,
        f: BoxFuture<'static, Result<Box<dyn Io>>>,
    ) -> Self {
        Self { done, endpoint, f }
    }

    pub async fn finalize(self) -> Result<Box<dyn Io>> {
        // This should never error since we are not polling the other side, so
        // the receiver should not be deallocated.
        self.done
            .send(())
            .expect("bug: the done signal receiver should not be deallocated");
        self.f.await
    }

    pub fn taget_endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

struct UpgradeSignal {
    endpoint_tx: Sender<Endpoint>,
    done_rx: Receiver<()>,
}

pub async fn serve(io: impl Io, config: Config) -> Result<SimplexMidHandshake> {
    let (done_tx, done_rx) = channel();
    let (endpoint_tx, endpoint_rx) = channel();

    let signal = Arc::new(Mutex::new(Some(UpgradeSignal {
        endpoint_tx,
        done_rx,
    })));

    let conn = Http::new().serve_connection(
        io,
        service_fn(move |req| {
            let config = config.clone();
            let signal = signal.clone();
            handler(req, config, signal).boxed()
        }),
    );

    let mut conn_fut = conn.without_shutdown().boxed();

    let endpoint = tokio::select! {
        _ = &mut conn_fut => {
            // There is no upgrade happens. The client is not a
            // simplex client;
            return Err(SimplexError::InvalidClient.into());
        }
        result = endpoint_rx => {
            match result {
                Ok(endpoint) => endpoint,
                Err(_) => unreachable!(),
            }
        }
    };

    Ok(SimplexMidHandshake::new(
        done_tx,
        endpoint,
        conn_fut
            .map_ok(|part| {
                let io: Box<dyn Io> = Box::new(ChainReadBufAndIo {
                    read_buf: part.read_buf,
                    io: part.io,
                });
                io
            })
            .err_into()
            .boxed(),
    ))
}

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
