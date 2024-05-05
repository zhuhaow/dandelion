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
    resolver: R,
) -> Result<QuicConnection> {
    Ok(QuicConnection {
        inner: client_connect(server, resolver).await?,
    })
}

pub async fn connect(endpoint: &Endpoint, connection: &QuicConnection) -> Result<QuicStream> {
    let (mut send, mut recv) = connection.inner.open_bi().await?;

    Ok(QuicStream::new(send, recv))
}
