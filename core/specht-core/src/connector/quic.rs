use super::Connector;
use crate::{
    endpoint::Endpoint,
    quic::{client::create_quic_connection, QuicMessage, QuicStream},
    resolver::Resolver,
    Result,
};
use anyhow::{bail, ensure};
use quinn::NewConnection;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct QuicConnector {
    connection: NewConnection,
}

pub async fn create_quic_connector<R: Resolver>(
    server: Endpoint,
    token: &str,
    resolver: &R,
) -> Result<QuicConnector> {
    Ok(QuicConnector {
        connection: create_quic_connection(server, token, resolver).await?,
    })
}

#[async_trait::async_trait]
impl Connector for QuicConnector {
    type Stream = QuicStream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let (mut send, mut recv) = self.connection.connection.open_bi().await?;

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
}
