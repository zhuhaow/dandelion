use crate::{
    endpoint::Endpoint,
    quic::{client::create_quic_connection as client_connect, QuicMessage, QuicStream},
    resolver::Resolver,
    Result,
};
use anyhow::{bail, ensure};
use quinn::Connection;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct QuicConnection {
    inner: Connection,
}

pub async fn create_quic_connection<R: Resolver>(
    server: Endpoint,
    token: &str,
    resolver: R,
) -> Result<QuicConnection> {
    Ok(QuicConnection {
        inner: client_connect(server, token, resolver).await?,
    })
}

pub async fn connect(endpoint: &Endpoint, connection: &QuicConnection) -> Result<QuicStream> {
    let (mut send, mut recv) = connection.inner.open_bi().await?;

    let endpoint_str = endpoint.to_string();
    let len = endpoint_str.as_bytes().len();
    ensure!(len < u8::MAX as usize, "endpoint {} is too long", endpoint);

    send.write_u8(len as u8).await?;
    send.write_all(endpoint_str.as_bytes()).await?;

    if recv.read_u8().await? != QuicMessage::Ok as u8 {
        bail!(
            "Failed to connect to endpoint {} from remote server",
            endpoint
        )
    }

    Ok(QuicStream::new(send, recv))
}
