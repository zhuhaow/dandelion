#![feature(trait_alias)]

pub mod acceptor;
pub mod config;
pub mod connector;
pub mod endpoint;
pub mod io;
pub mod resolver;
pub mod simplex;
pub mod tunnel;

#[derive(strum::Display, thiserror::Error, Debug)]
pub enum Error {
    Socks5Acceptor(#[from] acceptor::socks5::Socks5AcceptorError),

    Io(#[from] std::io::Error),

    Resolver(#[from] resolver::ResolverError),

    WebSocket(#[from] tungstenite::error::Error),

    Hyper(#[from] hyper::Error),

    Simplex(#[from] simplex::SimplexError),

    EndpointParse(#[from] endpoint::EndpointParseError),

    Ron(#[from] ron::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[no_mangle]
pub extern "C" fn test() {
    println!("Hello, world!");
}
