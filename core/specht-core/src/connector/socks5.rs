use super::Connector;
use crate::{endpoint::Endpoint, Result};
use anyhow::{bail, ensure, Context};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct Socks5Connector<C: Connector> {
    connector: C,
    next_hop: Endpoint,
}

impl<C: Connector> Socks5Connector<C> {
    pub fn new(connector: C, next_hop: Endpoint) -> Self {
        Self {
            connector,
            next_hop,
        }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for Socks5Connector<C> {
    type Stream = C::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        let mut io = self
            .connector
            .connect(&self.next_hop)
            .await
            .with_context(|| format!("Failed to connect to next hop {}", &self.next_hop))?;

        io.write_all(&[5, 1, 0]).await?;

        let mut buf = [0; 2];
        io.read_exact(&mut buf).await?;

        ensure!(buf[0] == 5, "Unsupported socks version: {}", buf[0]);
        ensure!(
            buf[1] == 0,
            "Server asked for auth method {} we don't support",
            buf[1]
        );

        let len = endpoint
            .hostname()
            .as_bytes()
            .len()
            .try_into()
            .with_context(|| "The socks5 protocol cannot support domain longer than 255 bytes.")?;
        io.write_all(&[5, 1, 0, 3, len]).await?;
        io.write_all(endpoint.hostname().as_bytes()).await?;
        io.write_all(&endpoint.port().to_be_bytes()).await?;

        let mut buf = [0; 4];
        io.read_exact(&mut buf).await?;
        ensure!(buf[0] == 5, "Unsupported socks version: {}", buf[0]);
        ensure!(
            buf[1] == 0,
            "Socks5 connection failed with status {}",
            buf[1]
        );
        ensure!(buf[2] == 0, "Not recognized reserved field");
        match buf[3] {
            1 => {
                let mut buf = [0; 6];
                io.read_exact(&mut buf).await?;
            }
            3 => {
                let len: usize = io.read_u8().await?.into();
                let mut buf = vec![0; len + 2];
                io.read_exact(&mut buf).await?;
            }
            4 => {
                let mut buf = [0; 18];
                io.read_exact(&mut buf).await?;
            }
            _ => {
                bail!("Not recognized address type {}", buf[3]);
            }
        }

        Ok(io)
    }
}
