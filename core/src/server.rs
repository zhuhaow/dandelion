use crate::{
    acceptor::socks5::Socks5Acceptor,
    connector::{Connector, ConnectorFactory, ConnectorFactoryWrapper},
    tunnel::Tunnel,
    Result,
};
use tokio::net::TcpListener;

pub struct Server {
    socks5_listener: TcpListener,
    connector_factory: ConnectorFactoryWrapper,
}

impl Server {
    pub async fn new<Factory: ConnectorFactory>(
        socks5_port: u16,
        connector_factory: Factory,
    ) -> Result<Self> {
        Ok(Self {
            socks5_listener: TcpListener::bind(("127.0.0.1", socks5_port)).await?,
            connector_factory: ConnectorFactoryWrapper::new(connector_factory),
        })
    }

    pub async fn accept(&self) -> Result<()> {
        self.accept_socks5().await
    }

    pub async fn accept_socks5(&self) -> Result<()> {
        loop {
            let (stream, _addr) = self.socks5_listener.accept().await?;

            let connector = self.connector_factory.build();

            tokio::spawn(async move {
                let handshake = Socks5Acceptor::new(stream).handshake().await?;
                let remote = connector.connect(handshake.target_endpoint()).await?;
                let local = handshake.finalize().await?;

                let mut tunnel = Tunnel::new(Box::new(local), remote);

                tunnel.process().await
            });
        }
    }
}
