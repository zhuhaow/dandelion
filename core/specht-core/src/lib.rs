#![warn(clippy::disallowed_type)]

pub mod acceptor;
pub mod connector;
pub mod endpoint;
pub mod geoip;
pub mod io;
pub mod resolver;
pub mod simplex;
pub mod tun;
pub mod utils;

pub use anyhow::Result;
