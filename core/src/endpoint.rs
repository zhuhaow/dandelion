use serde::Deserialize;
use std::{fmt::Display, net::SocketAddr, str::FromStr};

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum Endpoint {
    Addr(SocketAddr),
    Domain(String, u16),
}

impl Endpoint {
    pub fn new_from_domain(domain: &str, port: u16) -> Self {
        Endpoint::Domain(domain.to_owned(), port)
    }

    pub fn new_from_addr(addr: SocketAddr) -> Self {
        Endpoint::Addr(addr)
    }

    pub fn hostname(&self) -> String {
        match self {
            Endpoint::Addr(addr) => addr.ip().to_string(),
            Endpoint::Domain(d, _) => d.to_owned(),
        }
    }
}

#[derive(strum::Display, thiserror::Error, Debug)]
pub enum EndpointParseError {
    InvalidFormat,
    InvalidPort,
}

impl FromStr for Endpoint {
    type Err = EndpointParseError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        value.parse().map(Endpoint::new_from_addr).or_else(|_| {
            value
                .rsplit_once(':')
                .ok_or(EndpointParseError::InvalidFormat)
                .and_then(|(host, port)| {
                    Ok(Endpoint::new_from_domain(
                        host,
                        port.parse().map_err(|_| EndpointParseError::InvalidPort)?,
                    ))
                })
        })
    }
}

impl Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Endpoint::Addr(addr) => write!(f, "{}", addr),
            Endpoint::Domain(d, p) => write!(f, "{}:{}", d, p),
        }
    }
}
