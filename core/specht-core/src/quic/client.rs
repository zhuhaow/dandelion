use super::{QuicMessage, QuicStream};
use crate::{endpoint::Endpoint, resolver::Resolver, Result};
use anyhow::bail;
use futures::{future::select_ok, FutureExt};
use quinn::{ClientConfig, Connection, Endpoint as QuicEndpoint};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn create_quic_connection<R: Resolver>(
    server: Endpoint,
    token: &str,
    resolver: R,
) -> Result<Connection> {
    match server {
        // It's too unlikely there will be a ip cert, plus we need to verify if
        // ring will correctly validate this.
        Endpoint::Addr(addr) => bail!(
            "Cannot connect to remote with ip {}, domain is required for certificate validation",
            addr
        ),
        Endpoint::Domain(host, port) => {
            let addrs = resolver.lookup_ip(&host).await?;
            let host_ref = &host;
            let connection = select_ok(addrs.into_iter().map(|a| {
                async move {
                    match a {
                        std::net::IpAddr::V4(addr_v4) => Ok::<_, anyhow::Error>(
                            QuicEndpoint::client("0.0.0.0:0".parse().unwrap())?
                                .connect_with(
                                    ClientConfig::with_native_roots(),
                                    (addr_v4, port).into(),
                                    host_ref,
                                )?
                                .await?,
                        ),
                        std::net::IpAddr::V6(addr_v6) => {
                            Ok(QuicEndpoint::client("[::]:0".parse().unwrap())?
                                .connect_with(
                                    ClientConfig::with_native_roots(),
                                    (addr_v6, port).into(),
                                    host_ref,
                                )?
                                .await?)
                        }
                    }
                }
                .boxed()
            }))
            .await?
            .0;

            let (send, recv) = connection.open_bi().await?;

            let mut stream = QuicStream { send, recv };

            stream.write_all(token.as_bytes()).await?;
            if stream.read_u8().await? != QuicMessage::Ok as u8 {
                bail!("Authentication failed")
            }

            Ok(connection)
        }
    }
}
