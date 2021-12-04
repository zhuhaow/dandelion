use specht2_core::{
    acceptor::simplex::SimplexAcceptor, connector::tcp::TcpConnector, server::serve,
    simplex::Config, Result,
};
use std::net::SocketAddr;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Opt {
    #[structopt(long)]
    pub addr: SocketAddr,

    #[structopt(long)]
    pub path: String,

    #[structopt(long)]
    pub secret_key: String,

    #[structopt(long)]
    pub secret_value: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    serve(
        opt.addr,
        SimplexAcceptor::new(Config::new(opt.path, (opt.secret_key, opt.secret_value))),
        TcpConnector::default(),
    )
    .await
}
