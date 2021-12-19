use anyhow::{anyhow, Context};
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

    pub fn port(&self) -> u16 {
        match self {
            Endpoint::Addr(addr) => addr.port(),
            Endpoint::Domain(_, port) => *port,
        }
    }
}

impl FromStr for Endpoint {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        value.parse().map(Endpoint::new_from_addr).or_else(|_| {
            value
                .rsplit_once(':')
                .ok_or(anyhow!(
                    "Endpoint string not valid, most likely port is missing"
                ))
                .and_then(|(host, port)| {
                    Ok(Endpoint::new_from_domain(
                        host,
                        port.parse().context("Failed to parse port for endpoint")?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_from_str() {
        assert!(Endpoint::from_str("127.0.0.1:89").is_ok());
        assert!(Endpoint::from_str("127.0.0.1").is_err());
        assert!(Endpoint::from_str("google.com").is_err());
        assert!(Endpoint::from_str("google.com:443").is_ok());
        assert!(Endpoint::from_str("[fe::1]:443").is_ok());
    }
}
