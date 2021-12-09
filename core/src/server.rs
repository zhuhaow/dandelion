use crate::{
    acceptor::{simplex::SimplexAcceptor, socks5::Socks5Acceptor, Acceptor},
    connector::{
        simplex::SimplexConnector, tcp::TcpConnector, tls::TlsConector, BoxedConnector, Connector,
    },
    endpoint::Endpoint,
    simplex::Config,
    Error, Result,
};
use log::{debug, info, warn};
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
        secure: bool,
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

        info!("Server started");

        let addr = self.config.acceptor.server_addr();

        let listener = TcpListener::bind(addr).await?;

        info!("Server started listening on {}", addr);

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
                secure,
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
                        if secure {
                            BoxedConnector::new(TlsConector::new(TcpConnector::default()))
                        } else {
                            BoxedConnector::new(TcpConnector::default())
                        },
                    ))
                })
            }
        };

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Accepted a new connection from {}", addr);

            let acceptor = acceptor_fn();
            let connector = connector_fn();

            tokio::spawn(async move {
                let result = async move {
                    debug!("Start handshake");
                    let (endpoint, fut) = acceptor.do_handshake(stream).await?;
                    debug!("Accepted connection request to {}", endpoint);
                    let mut remote = connector.connect(&endpoint).await?;
                    debug!("Connected to {}", endpoint);
                    let mut local = fut.await?;
                    debug!("Forwarding data");
                    copy_bidirectional(&mut local, &mut remote).await?;
                    debug!("Done processing connection");

                    Ok::<_, Error>(())
                }
                .await;

                if let Err(err) = result {
                    warn!("Error happened when processing a connection: {}", err)
                } else {
                    debug!("Successfully processed connection");
                }
            });
        }
    }
}
