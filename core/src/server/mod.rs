pub mod config;
pub mod geoip;
pub mod privilege;

use self::privilege::PrivilegeHandler;
use crate::acceptor::http::HttpAcceptor;
use crate::acceptor::AsDynAcceptorArc;

use crate::tun::stack::create_stack;
use crate::{
    acceptor::{simplex::SimplexAcceptor, socks5::Socks5Acceptor},
    connector::Connector,
    server::config::{AcceptorConfig, ServerConfig},
    simplex::Config,
    Result,
};
use anyhow::bail;
use futures::future::{AbortRegistration, Abortable};
use futures::{future::try_join_all, stream::select_all, StreamExt, TryStreamExt};
use log::{debug, warn};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::{io::copy_bidirectional, net::TcpListener};
use tokio_stream::wrappers::TcpListenerStream;

#[derive(Default)]
struct Handles(Vec<JoinHandle<()>>);

impl Handles {
    pub fn push(&mut self, handle: JoinHandle<()>) {
        self.0.push(handle)
    }
}

impl Drop for Handles {
    fn drop(&mut self) {
        for handle in self.0.iter() {
            handle.abort();
        }
    }
}

pub struct Server<P: PrivilegeHandler> {
    config: ServerConfig,
    handler: P,
    route_traffic: bool,
}

impl<'a, P: PrivilegeHandler + Send + Sync + 'a> Server<P> {
    pub fn new(config: ServerConfig, handler: P, route_traffic: bool) -> Self {
        Self {
            config,
            handler,
            route_traffic,
        }
    }

    pub async fn serve(self, reg: AbortRegistration) -> Result<()> {
        let Self {
            config,
            handler: privilege_handler,
            route_traffic,
        } = self;

        if config.tun_enabled() && config.resolver.is_system() {
            bail!("Cannot use system resolver with TUN")
        }

        let mut task_handles = Handles::default();
        let resolver = config.resolver.get_resolver().await?;

        let privilege_handler_ref = &privilege_handler;
        let streams = try_join_all(config.acceptors.iter().map(|c| {
            let resolver = resolver.clone();
            async move {
                let listener_stream =
                    TcpListenerStream::new(TcpListener::bind(c.server_addr()).await?)
                        .map_err(Into::<anyhow::Error>::into);

                match c {
                    AcceptorConfig::Socks5 { .. } => Ok((
                        listener_stream,
                        Arc::new(Socks5Acceptor::default()).as_dyn_acceptor(),
                        None,
                    )),
                    AcceptorConfig::Simplex {
                        path,
                        secret_key,
                        secret_value,
                        ..
                    } => Ok((
                        listener_stream,
                        Arc::new(SimplexAcceptor::new(Config::new(
                            path.to_string(),
                            (secret_key.to_string(), secret_value.to_string()),
                        )))
                        .as_dyn_acceptor(),
                        None,
                    )),
                    AcceptorConfig::Http { .. } => Ok((
                        listener_stream,
                        Arc::new(HttpAcceptor::default()).as_dyn_acceptor(),
                        None,
                    )),
                    AcceptorConfig::Tun { subnet } => {
                        let device = privilege_handler_ref.create_tun_interface(subnet).await?;

                        let (fut, acceptor) = create_stack(device, *subnet, resolver).await?;

                        Ok::<_, anyhow::Error>((
                            listener_stream,
                            Arc::new(acceptor).as_dyn_acceptor(),
                            Some(fut),
                        ))
                    }
                }
            }
        }))
        .await?
        .into_iter()
        .fold(Vec::new(), |mut streams, (s, a, f)| {
            streams.push(s.map_ok(move |stream| (stream, a.clone())));

            if let Some(f) = f {
                task_handles.push(tokio::spawn(f));
            }

            streams
        });

        let mut listeners = select_all(streams);

        let connector = Arc::new(config.connector.get_connector(resolver).await?);

        if route_traffic {
            for c in config.acceptors.iter() {
                match c {
                    AcceptorConfig::Socks5 { addr } => {
                        privilege_handler.set_socks5_proxy(Some(*addr)).await?
                    }
                    AcceptorConfig::Simplex { .. } => {}
                    AcceptorConfig::Http { addr } => {
                        privilege_handler.set_http_proxy(Some(*addr)).await?
                    }
                    AcceptorConfig::Tun { subnet, .. } => {
                        privilege_handler
                            .set_dns(Some((subnet.iter().next().unwrap(), 53).into()))
                            .await?
                    }
                }
            }
        }

        let listen_fut = async move {
            while let Some(result) = listeners.next().await {
                let (stream, acceptor) = result?;

                let acceptor = acceptor.clone();
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

            Ok::<_, anyhow::Error>(())
        };

        let abortable = Abortable::new(listen_fut, reg);

        let result = abortable.await;

        if route_traffic {
            for c in config.acceptors.iter() {
                match c {
                    AcceptorConfig::Socks5 { .. } => {
                        privilege_handler.set_socks5_proxy(None).await?
                    }
                    AcceptorConfig::Simplex { .. } => {}
                    AcceptorConfig::Http { .. } => privilege_handler.set_http_proxy(None).await?,
                    AcceptorConfig::Tun { .. } => privilege_handler.set_dns(None).await?,
                }
            }
        }

        match result {
            Ok(res) => res,
            Err(_) => Ok(()),
        }
    }
}
