use specht2_core::{connector::tcp::TcpConnectorFactory, server::Server, Result};
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
    let connector_factory = TcpConnectorFactory::new();

    let server = Server::new(opt.socks5_port, connector_factory).await?;

    server.accept().await
}
