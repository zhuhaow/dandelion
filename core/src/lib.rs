#![feature(trait_alias)]

pub mod acceptor;
pub mod connector;
pub mod endpoint;
pub mod io;
pub mod resolver;
pub mod server;
pub mod simplex;
pub mod tunnel;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Socks5Acceptor(#[from] acceptor::socks5::Socks5AcceptorError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Resolver(#[from] resolver::ResolverError),
    #[error(transparent)]
    WebSocket(#[from] tungstenite::error::Error),
    #[error(transparent)]
    Hyper(#[from] hyper::Error),
    #[error(transparent)]
    Simplex(#[from] simplex::SimplexError),
    #[error(transparent)]
    EndpointParse(#[from] endpoint::EndpointParseError),
    #[error(transparent)]
    Ron(#[from] ron::Error),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[no_mangle]
pub extern "C" fn test() {
    println!("Hello, world!");
}
