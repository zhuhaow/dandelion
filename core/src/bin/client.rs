use std::net::SocketAddr;

use specht2_core::{
    acceptor::socks5::Socks5Acceptor, connector::tcp::TcpConnector, server::serve, Result,
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "specht2", about = "CLI version of the Specht2 client")]
struct Opt {
    #[structopt(long)]
    pub addr: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    serve(opt.addr, Socks5Acceptor::default(), TcpConnector::default()).await
}
