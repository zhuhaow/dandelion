use crate::{
    core::{
        endpoint::Endpoint,
        quic::{client::create_quic_connection as client_connect, QuicStream},
        resolver::Resolver,
    },
    Result,
};
use quinn::Connection;

#[derive(Debug)]
pub struct QuicConnection {
    inner: Connection,
}

pub async fn create_quic_connection<R: Resolver>(
    server: Endpoint,
    resolver: R,
    apln_protocols: Vec<Vec<u8>>,
) -> Result<QuicConnection> {
    Ok(QuicConnection {
        inner: client_connect(server, resolver, apln_protocols).await?,
    })
}

pub async fn connect(connection: &QuicConnection) -> Result<QuicStream> {
    let (send, recv) = connection.inner.open_bi().await?;

    Ok(QuicStream::new(send, recv))
}
