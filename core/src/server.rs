use crate::{
    acceptor::{simplex::SimplexAcceptor, socks5::Socks5Acceptor, Acceptor},
    connector::{
        boxed::BoxedConnectorFactory, simplex::SimplexConnectorFactory, tcp::TcpConnectorFactory,
        tls::TlsConnectorFactory, Connector, ConnectorFactory,
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
        next: Box<ConnectorConfig>,
    },
    Tls(Box<ConnectorConfig>),
}

impl ConnectorConfig {
    fn get_factory(&self) -> BoxedConnectorFactory {
        match self {
            ConnectorConfig::Direct => BoxedConnectorFactory::new(TcpConnectorFactory {}),
            ConnectorConfig::Simplex {
                endpoint,
                path,
                secret_key,
                secret_value,
                next,
            } => BoxedConnectorFactory::new(SimplexConnectorFactory::new(
                next.get_factory(),
                endpoint.clone(),
                Config::new(
                    path.to_owned(),
                    (secret_key.to_owned(), secret_value.to_owned()),
                ),
            )),
            ConnectorConfig::Tls(c) => {
                BoxedConnectorFactory::new(TlsConnectorFactory::new(c.get_factory()))
            }
        }
    }
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

        let connector_factory = self.config.connector.get_factory();

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Accepted a new connection from {}", addr);

            let acceptor = acceptor_fn();
            let connector = connector_factory.build();

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

#[cfg(test)]
mod test {
    use super::ServerConfig;
    use crate::Result;
    use rstest::rstest;
    use std::{env, fs::read_to_string, path::Path};

    #[rstest]
    #[case("local.ron")]
    #[case("remote.ron")]
    fn config_file(#[case] filename: &str) -> Result<()> {
        let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("config")
            .join(filename);
        let _config: ServerConfig = ron::de::from_str(&read_to_string(path)?)?;
        Ok(())
    }
}
