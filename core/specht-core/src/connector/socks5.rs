use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::{bail, ensure, Context};
use futures::Future;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn connect<I: Io, F: Future<Output = Result<I>>, C: Fn(&Endpoint) -> F>(
    connector: C,
    endpoint: &Endpoint,
    next_hop: &Endpoint,
) -> Result<I> {
    let mut s = connector(next_hop)
        .await
        .with_context(|| format!("Failed to connect to next hop {}", &next_hop))?;

    s.write_all(&[5, 1, 0]).await?;

    let mut buf = [0; 2];
    s.read_exact(&mut buf).await?;

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
    s.write_all(&[5, 1, 0, 3, len]).await?;
    s.write_all(endpoint.hostname().as_bytes()).await?;
    s.write_all(&endpoint.port().to_be_bytes()).await?;

    let mut buf = [0; 4];
    s.read_exact(&mut buf).await?;
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
            s.read_exact(&mut buf).await?;
        }
        3 => {
            let len: usize = s.read_u8().await?.into();
            let mut buf = vec![0; len + 2];
            s.read_exact(&mut buf).await?;
        }
        4 => {
            let mut buf = [0; 18];
            s.read_exact(&mut buf).await?;
        }
        _ => {
            bail!("Not recognized address type {}", buf[3]);
        }
    }

    Ok(s)
}
