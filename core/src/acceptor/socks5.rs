use crate::{Endpoint, Io, Result};
use std::{
    net::{IpAddr, SocketAddr},
    string::FromUtf8Error,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

pub struct Socks5Acceptor<T: Io> {
    io: T,
}

impl<T: Io> Socks5Acceptor<T> {
    pub fn new(io: T) -> Self {
        Self { io }
    }
}

impl<T: Io> Socks5Acceptor<T> {
    pub async fn handshake(self) -> Result<Socks5MidHandshake<T>> {
        let mut io = self.io;

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
        let mut buf = vec![0, buf[1]];
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

        Ok(Socks5MidHandshake {
            _io: io,
            _endpoint: endpoint,
        })
    }
}

pub struct Socks5MidHandshake<T: Io> {
    _io: T,
    _endpoint: Endpoint,
}
