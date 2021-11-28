#![feature(trait_alias)]
pub mod acceptor;

use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite};

pub trait Io = AsyncRead + AsyncWrite + Unpin;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("socks5 acceptor error: {0}")]
    Socks5(#[from] acceptor::socks5::Socks5AcceptorError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
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

#[no_mangle]
pub extern "C" fn test() {
    println!("Hello, world!");
}
