use super::{QuicMessage, QuicStream};
use crate::Result;
use anyhow::bail;
use futures::stream::once;
use futures::{FutureExt, Stream, StreamExt, TryStreamExt};
use quinn::{Endpoint, NewConnection, ServerConfig};
use rustls::{Certificate, PrivateKey};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};

pub fn create_server(
    addr: SocketAddr,
    cert: Certificate,
    key: PrivateKey,
    token: String,
) -> Result<impl Stream<Item = Result<QuicStream>>> {
    let config = ServerConfig::with_single_cert(vec![cert], key)?;

    let (_, incoming) = Endpoint::server(config, addr)?;

    Ok(incoming
        .map(move |conn| {
            let token = token.clone();

            async move {
                let connection = conn.await?;
                let NewConnection { mut bi_streams, .. } = connection;
                let (send, recv) = bi_streams
                    .next()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("Connection closed without authentication"))??;
                let mut stream = QuicStream { send, recv };
                let mut buf = vec![0_u8; token.as_bytes().len()];
                stream.read_exact(&mut buf).await?;

                if token != String::from_utf8(buf)? {
                    bail!("Authentication failed");
                }

                stream.write_u8(QuicMessage::Ok as u8).await?;

                Ok::<_, anyhow::Error>(bi_streams)
            }
        })
        // add timeout
        .map(|f| {
            timeout(Duration::from_secs(10), f).map(|r| match r {
                Ok(result) => result,
                Err(err) => Err(err.into()),
            })
        })
        // We can process authentication concurrently
        .buffer_unordered(30)
        // Now each item of the stream is a Result of stream.
        .flat_map_unordered(None, |r| match r {
            Ok(s) => s
                .map_ok(|(send, recv)| QuicStream { send, recv })
                .map_err(|e| e.into())
                .boxed(),
            Err(err) => once(async move { Err(err) }).boxed(),
        }))
}
