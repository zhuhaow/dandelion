#![feature(trait_alias)]
pub mod acceptor;
pub mod connector;
pub mod io;
pub mod resolver;
pub mod server;
pub mod tunnel;

use std::net::SocketAddr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("socks5 acceptor error: {0}")]
    Socks5Acceptor(#[from] acceptor::socks5::Socks5AcceptorError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("resolver error: {0}")]
    Resolver(#[from] resolver::ResolverError),

    #[error("ws error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::error::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

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
}

impl ToString for Endpoint {
    fn to_string(&self) -> String {
        match self {
            Endpoint::Addr(addr) => addr.to_string(),
            Endpoint::Domain(d, p) => format!("{}:{}", d, p),
        }
    }
}

#[no_mangle]
pub extern "C" fn test() {
    println!("Hello, world!");
}
