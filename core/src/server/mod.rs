pub mod config;
pub mod geoip;

use crate::{
    connector::{Connector, ConnectorFactory},
    server::config::ServerConfig,
    Result,
};
use futures::{future::try_join_all, stream::select_all, TryStreamExt};
use log::{debug, info, warn};
use tokio::{io::copy_bidirectional, net::TcpListener};
use tokio_stream::{wrappers::TcpListenerStream, StreamExt};

pub struct Server {
    config: ServerConfig,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    pub async fn serve(&self) -> Result<()> {
        info!("Server started");

        let mut listeners = select_all(
            try_join_all(
                self.config
                    .acceptors
                    .iter()
                    .map(|c| TcpListener::bind(c.server_addr())),
            )
            .await?
            .into_iter()
            .map(TcpListenerStream::new)
            .zip(self.config.acceptors.iter())
            .map(|(s, c)| s.map_ok(move |stream| (stream, c))),
        );

        let connector_factory = self.config.connector.get_factory().await?;

        while let Some(result) = listeners.next().await {
            let (stream, acceptor_config) = result?;

            let acceptor = acceptor_config.get_acceptor();
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
        Ok(())
    }
}
