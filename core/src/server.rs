use crate::{
    acceptor::{simplex::SimplexAcceptor, socks5::Socks5Acceptor, Acceptor},
    connector::{simplex::SimplexConnector, tcp::TcpConnector, BoxedConnector, Connector},
    endpoint::Endpoint,
    simplex::Config,
    Error, Result,
};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use std::net::SocketAddr;
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    acceptor: AcceptorConfig,
    connector: ConnectorConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub enum AcceptorConfig {
    Socks5 {
        addr: SocketAddr,
    },
    Simplex {
        addr: SocketAddr,
        path: String,
        secret_key: String,
        secret_value: String,
    },
}

impl AcceptorConfig {
    fn server_addr(&self) -> &SocketAddr {
        match self {
            AcceptorConfig::Socks5 { addr } => addr,
            AcceptorConfig::Simplex {
                addr,
                path: _,
                secret_key: _,
                secret_value: _,
            } => addr,
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub enum ConnectorConfig {
    Direct,
    Simplex {
        #[serde_as(as = "DisplayFromStr")]
        endpoint: Endpoint,
        path: String,
        secret_key: String,
        secret_value: String,
    },
}

pub struct Server {
    config: ServerConfig,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    pub async fn serve(&self) -> Result<()> {
        // TODO: we can probably remove Acceptor and Connector and simply replace it with a lambda.

        let addr = self.config.acceptor.server_addr();

        let listener = TcpListener::bind(addr).await?;

        let acceptor_fn: Box<dyn Fn() -> Box<dyn Acceptor<TcpStream>>> = match self.config.acceptor
        {
            AcceptorConfig::Socks5 { addr: _ } => Box::new(|| Box::new(Socks5Acceptor {})),
            AcceptorConfig::Simplex {
                addr: _,
                ref path,
                ref secret_key,
                ref secret_value,
            } => {
                let config = Config::new(
                    path.to_string(),
                    (secret_key.to_string(), secret_value.to_string()),
                );
                Box::new(move || Box::new(SimplexAcceptor::new(config.clone())))
            }
        };

        let connector_fn: Box<dyn Fn() -> BoxedConnector> = match self.config.connector {
            ConnectorConfig::Direct => Box::new(|| BoxedConnector::new(TcpConnector {})),
            ConnectorConfig::Simplex {
                ref endpoint,
                ref path,
                ref secret_key,
                ref secret_value,
            } => {
                let config = Config::new(
                    path.to_string(),
                    (secret_key.to_string(), secret_value.to_string()),
                );
                let endpoint = endpoint.clone();

                Box::new(move || {
                    BoxedConnector::new(SimplexConnector::new(
                        endpoint.clone(),
                        config.clone(),
                        TcpConnector {},
                    ))
                })
            }
        };

        loop {
            let (stream, _addr) = listener.accept().await?;
            let acceptor = acceptor_fn();
            let connector = connector_fn();

            tokio::spawn(async move {
                let (endpoint, fut) = acceptor.do_handshake(stream).await?;
                let mut remote = connector.connect(&endpoint).await?;
                let mut local = fut.await?;
                copy_bidirectional(&mut local, &mut remote).await?;

                Ok::<_, Error>(())
            });
        }
    }
}
