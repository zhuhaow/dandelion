pub mod config;
pub mod geoip;
pub mod privilege;

use self::privilege::PrivilegeHandler;
use crate::{
    connector::Connector,
    server::config::{AcceptorConfig, ServerConfig},
    Result,
};
use futures::{
    future::try_join_all,
    stream::{select_all, BoxStream},
    StreamExt, TryStreamExt,
};
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};
use tokio_stream::wrappers::TcpListenerStream;

pub struct Server<P: PrivilegeHandler> {
    config: ServerConfig,
    handler: P,
}

impl<P: PrivilegeHandler> Server<P> {
    pub fn new(config: ServerConfig, handler: P) -> Self {
        Self { config, handler }
    }

    /// If route_traffic is set to true, we need to call `clear_traffic_routing`
    /// manually to clear all routing config.
    ///
    /// This is necessary since the server won't `stop` unless we don't poll it.
    /// So the server cannot clear the routing when the it actually stops.
    ///
    /// We cannot put it in deinit since it's async.
    pub async fn serve(&self, route_traffic: bool) -> Result<()> {
        info!("Server started");

        let mut listeners = select_all(
            try_join_all(self.config.acceptors.iter().map(|c| async move {
                Ok::<BoxStream<Result<TcpStream>>, anyhow::Error>(
                    TcpListenerStream::new(TcpListener::bind(c.server_addr()).await?)
                        .map_err(Into::into)
                        .boxed(),
                )
            }))
            .await?
            .into_iter()
            .zip(self.config.acceptors.iter())
            .map(|(s, c)| s.map_ok(move |stream| (stream, c))),
        );

        let tun_config = self
            .config
            .acceptors
            .iter()
            .find(|c| matches!(c, AcceptorConfig::Tun { .. }));

        let resolver = self.config.resolver.get_resolver();
        let connector = Arc::new(self.config.connector.get_connector(resolver).await?);

        if route_traffic {
            self.redirect_traffic().await?;
        }

        while let Some(result) = listeners.next().await {
            let (stream, acceptor_config) = result?;

            let acceptor = acceptor_config.get_acceptor();
            let connector = connector.clone();

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

    async fn redirect_traffic(&self) -> Result<()> {
        for c in self.config.acceptors.iter() {
            match c {
                AcceptorConfig::Socks5 { addr } => {
                    self.handler.set_socks5_proxy(Some(*addr)).await?
                }
                AcceptorConfig::Simplex { .. } => {}
                AcceptorConfig::Http { addr } => self.handler.set_http_proxy(Some(*addr)).await?,
                AcceptorConfig::Tun { subnet, .. } => {
                    self.handler
                        .set_dns(Some((subnet.iter().next().unwrap(), 53).into()))
                        .await?
                }
            }
        }

        Ok(())
    }

    pub async fn clear_traffic_routing(&self) -> Result<()> {
        for c in self.config.acceptors.iter() {
            match c {
                AcceptorConfig::Socks5 { .. } => self.handler.set_socks5_proxy(None).await?,
                AcceptorConfig::Simplex { .. } => {}
                AcceptorConfig::Http { .. } => self.handler.set_http_proxy(None).await?,
                AcceptorConfig::Tun { .. } => self.handler.set_dns(None).await?,
            }
        }

        Ok(())
    }
}
