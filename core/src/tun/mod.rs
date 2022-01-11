use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

pub mod acceptor;
pub mod device;
pub mod dns;
mod route;
pub mod stack;
mod translator;

fn ipv4_addr_to_socketaddr(addr: Ipv4Addr, port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(addr, port))
}
