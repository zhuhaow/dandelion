use super::{Acceptor, HandshakeResult};
use crate::{endpoint::Endpoint, io::Io, Error, Result};
use futures::{Future, FutureExt};
use std::{
    net::{IpAddr, SocketAddr},
    string::FromUtf8Error,
};
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
                Ok::<_, Error>(io)
            }
            .boxed(),
        ))
    }
}

#[derive(Debug, strum::Display)]
pub enum Socks5AcceptorError {
    UnsupportedVersion,
    InvalidMethodCount,
    UnsupportedAuthMethod,
    UnsupportedCommand,
    UnsupportedAddressType,
    DomainError(FromUtf8Error),
}

impl std::error::Error for Socks5AcceptorError {}

pub async fn handshake(
    mut io: impl Io,
) -> Result<(Endpoint, impl Future<Output = Result<impl Io>>)> {
    // Read hello
    let mut buf = [0; 2];
    io.read_exact(&mut buf).await?;

    if buf[0] != 5 {
        return Err(Socks5AcceptorError::UnsupportedVersion.into());
    }

    if buf[1] == 0 {
        return Err(Socks5AcceptorError::InvalidMethodCount.into());
    }

    // Read requested methods
    let mut buf = vec![0; buf[1].into()];
    io.read_exact(&mut buf).await?;

    // Check if there is no auth requested since that's the only one we support
    if !buf.iter().any(|x| *x == 0) {
        return Err(Socks5AcceptorError::UnsupportedAuthMethod.into());
    }

    // Send back the method we support.
    let buf: [u8; 2] = [5, 0];
    io.write_all(&buf).await?;

    // Read requested endpoint
    let mut buf = [0; 4];
    io.read_exact(&mut buf).await?;

    if buf[0] != 5 {
        return Err(Socks5AcceptorError::UnsupportedVersion.into());
    }

    if buf[1] != 1 {
        return Err(Socks5AcceptorError::UnsupportedCommand.into());
    }

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
            let mut buf = [0; 1];
            io.read_exact(&mut buf).await?;

            let mut buf = vec![0; buf[0].into()];

            io.read_exact(&mut buf).await?;
            let domain = String::from_utf8(buf).map_err(Socks5AcceptorError::DomainError)?;
            IpOrDomain::Domain(domain)
        }
        4 => {
            let mut buf = [0; 16];
            io.read_exact(&mut buf).await?;
            IpOrDomain::Ip(IpAddr::from(buf))
        }
        _ => return Err(Socks5AcceptorError::UnsupportedAddressType.into()),
    };

    let mut buf = [0; 2];
    io.read_exact(&mut buf).await?;
    let port = u16::from_be_bytes(buf);

    let endpoint = match ip_or_domain {
        IpOrDomain::Domain(d) => Endpoint::new_from_domain(&d, port),
        IpOrDomain::Ip(ip) => Endpoint::new_from_addr(SocketAddr::new(ip, port)),
    };

    Ok((endpoint, async move {
        io.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        Ok::<_, Error>(io)
    }))
}
