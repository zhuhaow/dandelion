use super::{Acceptor, HandshakeResult};
use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::{bail, ensure, Context};
use futures::{Future, FutureExt};
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct Socks5Acceptor {}

#[async_trait::async_trait]
impl<I: Io> Acceptor<I> for Socks5Acceptor {
    async fn do_handshake(&self, io: I) -> HandshakeResult {
        let (endpoint, fut) = handshake(io).await?;
        Ok((
            endpoint,
            async move {
                let io: Box<dyn Io> = Box::new(fut.await?);
                Ok(io)
            }
            .boxed(),
        ))
    }
}

pub async fn handshake(
    mut io: impl Io,
) -> Result<(Endpoint, impl Future<Output = Result<impl Io>>)> {
    // Read hello
    let mut buf = [0; 2];
    io.read_exact(&mut buf).await?;

    ensure!(buf[0] == 5, "Unsupported socks version: {}", buf[0]);

    ensure!(
        buf[1] != 0,
        "Invalid socks5 auth method count, should not be 0"
    );

    // Read requested methods
    let mut buf = vec![0; buf[1].into()];
    io.read_exact(&mut buf).await?;

    // Check if there is no auth requested since that's the only one we support
    ensure!(
        buf.iter().any(|x| *x == 0),
        "Only no auth is supported, but it's not requested in handshake"
    );

    // Send back the method we support.
    let buf: [u8; 2] = [5, 0];
    io.write_all(&buf).await?;

    // Read requested endpoint
    let mut buf = [0; 4];
    io.read_exact(&mut buf).await?;

    ensure!(buf[0] == 5, "Unsupported socks version: {}", buf[0]);

    ensure!(
        buf[1] == 1,
        "Invalid socks5 command: {}, only 1 is supported",
        buf[1]
    );

    enum IpOrDomain {
        Ip(IpAddr),
        Domain(String),
    }

    let ip_or_domain = match buf[3] {
        1 => {
            let mut buf = [0; 4];
            io.read_exact(&mut buf).await?;
            IpOrDomain::Ip(IpAddr::from(buf))
        }
        3 => {
            let len: usize = io.read_u8().await?.into();
            let mut buf = vec![0; len];
            io.read_exact(&mut buf).await?;
            let domain = String::from_utf8(buf)
                .context("The socks5 client is not sending a valid domain")?;
            IpOrDomain::Domain(domain)
        }
        4 => {
            let mut buf = [0; 16];
            io.read_exact(&mut buf).await?;
            IpOrDomain::Ip(IpAddr::from(buf))
        }
        t => bail!("Unsupported address type {}", t),
    };

    let port = io.read_u16().await?;

    let endpoint = match ip_or_domain {
        IpOrDomain::Domain(d) => Endpoint::new_from_domain(&d, port),
        IpOrDomain::Ip(ip) => Endpoint::new_from_addr(SocketAddr::new(ip, port)),
    };

    Ok((endpoint, async move {
        io.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        Ok(io)
    }))
}
