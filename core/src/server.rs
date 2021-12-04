use crate::{
    acceptor::{Acceptor, MidHandshake},
    connector::Connector,
    tunnel::tunnel,
    Result,
};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

pub async fn serve(
    addr: SocketAddr,
    acceptor: impl Acceptor<Input = TcpStream>,
    connector: impl Connector,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _addr) = listener.accept().await?;

        let connector = connector.clone();
        let acceptor = acceptor.clone();

        tokio::spawn(async move {
            let handshake = acceptor.handshake(stream).await?;
            let endpoint = handshake.target_endpoint().clone();
            let remote = connector.connect(&endpoint).await?;
            let local = handshake.finalize().await?;

            tunnel(local, remote).await
        });
    }
}
