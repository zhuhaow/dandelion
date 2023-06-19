use crate::engine::{AcceptorConfig, Engine};
use futures::{future::select_all, Future, FutureExt, TryStreamExt};
use specht_core::{
    acceptor::{http, socks5},
    endpoint::Endpoint,
    io::Io,
    Error, Result,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};
use tokio_stream::wrappers::TcpListenerStream;

pub struct Instance {
    engine: Arc<Engine>,
}

impl Instance {
    pub async fn load_config(name: impl AsRef<str>, code: impl AsRef<str>) -> Result<Self> {
        let engine = Arc::new(Engine::load_config(name, code).await?);

        Ok(Self { engine })
    }

    pub async fn run(&self) -> Result<()> {
        select_all(self.engine.get_acceptors().iter().map(|c| {
            match c {
                AcceptorConfig::Socks5(addr, handler) => handle_acceptors(
                    addr,
                    socks5::handshake,
                    self.engine.clone(),
                    handler.to_owned(),
                )
                .boxed_local(),
                AcceptorConfig::Http(addr, handler) => handle_acceptors(
                    addr,
                    http::handshake,
                    self.engine.clone(),
                    handler.to_owned(),
                )
                .boxed_local(),
            }
        }))
        .await
        .0
    }
}

pub async fn handle_acceptors<
    F: Future<Output = Result<(Endpoint, impl Future<Output = Result<impl Io>>)>> + 'static,
>(
    addr: &SocketAddr,
    handshake: fn(TcpStream) -> F,
    engine: Arc<Engine>,
    eval_fn: String,
) -> Result<()> {
    while let Some(io) = TcpListenerStream::new(TcpListener::bind(addr).await?)
        .map_err(Into::<Error>::into)
        .try_next()
        .await?
    {
        let engine = engine.clone();
        let eval_fn = eval_fn.clone();

        tokio::task::spawn_local(async move {
            if let Err(e) = async {
                let (endpoint, fut) = handshake(io).await?;

                let mut remote = engine.run_handler(eval_fn, endpoint).await?.into_inner();

                let mut local = fut.await?;

                copy_bidirectional(&mut local, &mut remote).await?;

                Ok::<(), Error>(())
            }
            .await
            {
                tracing::error!("Failed to handle connection {}", e)
            }
        });
    }

    Ok(())
}
