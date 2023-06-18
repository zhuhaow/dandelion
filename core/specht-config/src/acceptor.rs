use std::{net::SocketAddr, sync::Arc};

use futures::{Future, TryStreamExt};
use specht_core::{endpoint::Endpoint, io::Io, Error, Result};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};
use tokio_stream::wrappers::TcpListenerStream;

use crate::engine::ConfigEngine;

pub async fn handle_acceptors<
    F: Future<Output = Result<(Endpoint, impl Future<Output = Result<impl Io>>)>> + 'static,
>(
    addr: &SocketAddr,
    handshake: fn(TcpStream) -> F,
    engine: Arc<ConfigEngine>,
    eval_fn: String,
) -> Result<()> {
    while let Some(io) = TcpListenerStream::new(TcpListener::bind(addr).await?)
        .map_err(Into::<Error>::into)
        .try_next()
        .await?
    {
        let engine = engine.clone();
        let eval_fn = eval_fn.clone();

        // tokio::task::spawn_local(async move {
        //     let (endpoint, fut) = handshake(io).await?;

        //     let remote = engine.run_handler(eval_fn, endpoint.into()).await?;
        //     let mut remote = connnector.connect().await?;

        //     let mut local = fut.await?;

        //     copy_bidirectional(&mut local, &mut remote).await?;

        //     Ok::<(), Error>(())
        // });
    }

    Ok(())
}
