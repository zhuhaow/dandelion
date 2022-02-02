pub mod config;
pub mod geoip;
pub mod privilege;

use self::privilege::PrivilegeHandler;
use crate::config::{AcceptorConfig, ServerConfig};
use anyhow::{bail, Context};
use futures::{
    future::{try_join_all, AbortRegistration, Abortable},
    stream::select_all,
    Stream, StreamExt, TryStreamExt,
};
use rand::Rng;
use specht_core::{
    acceptor::{
        http::HttpAcceptor, simplex::SimplexAcceptor, socks5::Socks5Acceptor, AsDynAcceptorArc,
    },
    connector::Connector,
    simplex::Config,
    tun::stack::create_stack,
    Result,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_stream::wrappers::TcpListenerStream;
use tracing::{debug, info_span, warn, Instrument};

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
}

impl<'a, P: PrivilegeHandler + Send + Sync + 'a> Server<P> {
    pub fn new(config: ServerConfig, handler: P) -> Self {
        Self { config, handler }
    }

    pub async fn serve(self, reg: AbortRegistration) -> Result<()> {
        let Self {
            config,
            handler: privilege_handler,
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
                async fn create_stream(
                    addr: SocketAddr,
                ) -> Result<impl Stream<Item = Result<TcpStream>>> {
                    Ok(TcpListenerStream::new(TcpListener::bind(addr).await?)
                        .map_err(Into::<anyhow::Error>::into))
                }

                match c {
                    AcceptorConfig::Socks5 { .. } => Ok((
                        create_stream(c.server_addr()).await?,
                        Arc::new(Socks5Acceptor::default()).as_dyn_acceptor(),
                        None,
                    )),
                    AcceptorConfig::Simplex {
                        path,
                        secret_key,
                        secret_value,
                        ..
                    } => Ok((
                        create_stream(c.server_addr()).await?,
                        Arc::new(SimplexAcceptor::new(Config::new(
                            path.to_string(),
                            (secret_key.to_string(), secret_value.to_string()),
                        )))
                        .as_dyn_acceptor(),
                        None,
                    )),
                    AcceptorConfig::Http { .. } => Ok((
                        create_stream(c.server_addr()).await?,
                        Arc::new(HttpAcceptor::default()).as_dyn_acceptor(),
                        None,
                    )),
                    AcceptorConfig::Tun { subnet } => {
                        let device = privilege_handler_ref.create_tun_interface(subnet).await?;

                        let (fut, acceptor) = create_stack(device, *subnet, resolver).await?;

                        Ok::<_, anyhow::Error>((
                            create_stream(c.server_addr()).await?,
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

        let connector = config.connector.get_connector(resolver).await?;

        for c in config.acceptors.iter() {
            match c {
                AcceptorConfig::Socks5 { addr, managed } => {
                    if *managed {
                        privilege_handler.set_socks5_proxy(Some(*addr)).await?
                    }
                }
                AcceptorConfig::Simplex { .. } => {}
                AcceptorConfig::Http { addr, managed } => {
                    if *managed {
                        privilege_handler.set_http_proxy(Some(*addr)).await?
                    }
                }
                AcceptorConfig::Tun { subnet, .. } => {
                    privilege_handler
                        .set_dns(Some((subnet.ip(), 53).into()))
                        .await?
                }
            }
        }

        let listen_fut = async move {
            while let Some(result) = listeners.next().await {
                let (stream, acceptor) = result?;

                let acceptor = acceptor.clone();
                let connector = connector.clone();

                tokio::spawn(
                    async move {
                        let result = async move {
                            let (endpoint, fut) = acceptor
                                .do_handshake(stream)
                                .await
                                .context("Failed to get connect request from acceptor")?;
                            let mut remote =
                                connector.connect(&endpoint).await.with_context(|| {
                                    format!("Failed to connect to endpoint {}", endpoint)
                                })?;
                            let mut local = fut.await.with_context(|| {
                                format!("Failed to finish handshake with local which requests to connect to {}", endpoint)
                            })?;
                            copy_bidirectional(&mut local, &mut remote).await.with_context(|| {
                                format!("Failed to forward data with endpoint {}", endpoint)
                            })?;
                            debug!("Done processing connection to {}", endpoint);

                            Ok::<_, anyhow::Error>(())
                        }
                        .await;

                        if let Err(err) = result {
                            warn!("Error happened when processing a connection: {:#}", err)
                        }
                    }
                    .instrument(info_span!(
                        "connection",
                        id = rand::thread_rng().gen::<u32>()
                    )),
                );
            }

            Ok::<_, anyhow::Error>(())
        };

        let abortable = Abortable::new(listen_fut, reg);

        let result = abortable.await;

        for c in config.acceptors.iter() {
            match c {
                AcceptorConfig::Socks5 { managed, .. } => {
                    if *managed {
                        privilege_handler.set_socks5_proxy(None).await?
                    }
                }
                AcceptorConfig::Simplex { .. } => {}
                AcceptorConfig::Http { managed, .. } => {
                    if *managed {
                        privilege_handler.set_http_proxy(None).await?
                    }
                }
                AcceptorConfig::Tun { .. } => privilege_handler.set_dns(None).await?,
            }
        }

        match result {
            Ok(res) => res,
            Err(_) => Ok(()),
        }
    }
}
