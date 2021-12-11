#![feature(trait_alias)]

pub mod acceptor;
pub mod connector;
pub mod endpoint;
pub mod geoip;
pub mod io;
pub mod resolver;
pub mod server;
pub mod simplex;
pub mod tunnel;

pub use anyhow::Result;

#[no_mangle]
pub extern "C" fn test() {
    println!("Hello, world!");
}
