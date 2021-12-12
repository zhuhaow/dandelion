pub mod config;
pub mod geoip;

use crate::{
    acceptor::{simplex::SimplexAcceptor, socks5::Socks5Acceptor, Acceptor},
    connector::{Connector, ConnectorFactory},
    server::config::{AcceptorConfig, ServerConfig},
    simplex::Config,
    Result,
};
use log::{debug, info, warn};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};

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

        let connector_factory = self.config.connector.get_factory().await?;

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

                    Ok::<_, anyhow::Error>(())
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
