use std::net::SocketAddr;

use specht2_core::{
    acceptor::socks5::Socks5Acceptor, connector::tcp::TcpConnector, server::serve, Result,
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "specht2", about = "CLI version of the Specht2 client")]
struct Opt {
    #[structopt(long)]
    socks5_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    serve(
        SocketAddr::new("127.0.0.1".parse().unwrap(), opt.socks5_port),
        Socks5Acceptor::default(),
        TcpConnector::default(),
    )
    .await
}
