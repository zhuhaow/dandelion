use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::{bail, ensure, Context};
use bytes::{Bytes, BytesMut};
use futures::Future;
use http::{Request, StatusCode};
use httparse::{Response, Status, EMPTY_HEADER};
use hyper::Body;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

async fn connect<I: Io, F: Future<Output = Result<I>>, C: Fn(&Endpoint) -> F>(
    connector: C,
    endpoint: &Endpoint,
    next_hop: &Endpoint,
) -> Result<I> {
    debug!("Begin HTTP CONNECT handshake");

    let mut s = connector(next_hop)
        .await
        .with_context(|| format!("Failed to connect to next hop {}", next_hop))?;

    s.write_all(
        format!(
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n",
            endpoint, endpoint
        )
        .as_bytes(),
    )
    .await
    .with_context(|| {
        format!(
            "Failed to send CONNECT request to server {} connecting to {}",
            next_hop, endpoint
        )
    })?;

    // We should not have a huge response
    let mut buf = BytesMut::with_capacity(4196);

    while s
        .read_buf(&mut buf)
        .await
        .with_context(|| format!("Failed to read CONNECT response from server {}", next_hop))?
        != 0
    {
        let mut headers = [EMPTY_HEADER; 64];
        let mut res = Response::new(&mut headers);

        if res.parse(&buf)?.is_complete() {
            if res.code == Some(200) {
                break;
            } else {
                bail!(
                    "Failed to CONNECT to {} from server {}, got error response {}",
                    endpoint,
                    next_hop,
                    std::str::from_utf8(&buf)?
                )
            }
        }
    }

    debug!("Finished HTTP CONNECT handshake");

    Ok(s)
}
