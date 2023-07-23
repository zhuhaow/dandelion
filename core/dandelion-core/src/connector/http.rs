use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::{bail, Context};
use bytes::BytesMut;
use httparse::{Response, EMPTY_HEADER};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

pub async fn connect(endpoint: &Endpoint, mut nexthop: impl Io) -> Result<impl Io> {
    debug!("Begin HTTP CONNECT handshake");

    nexthop
        .write_all(
            format!(
                "CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n",
                endpoint, endpoint
            )
            .as_bytes(),
        )
        .await
        .with_context(|| format!("Failed to send CONNECT request to connect to {}", endpoint))?;

    // We should not have a huge response
    let mut buf = BytesMut::with_capacity(4196);

    while nexthop
        .read_buf(&mut buf)
        .await
        .context("Failed to read CONNECT response")?
        != 0
    {
        let mut headers = [EMPTY_HEADER; 64];
        let mut res = Response::new(&mut headers);

        if res.parse(&buf)?.is_complete() {
            if res.code == Some(200) {
                break;
            } else {
                bail!(
                    "Failed to CONNECT to {}, got error response {}",
                    endpoint,
                    std::str::from_utf8(&buf)?
                )
            }
        }
    }

    debug!("Finished HTTP CONNECT handshake");

    Ok(nexthop)
}
