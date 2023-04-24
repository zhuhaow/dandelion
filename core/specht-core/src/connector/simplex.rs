use crate::{
    endpoint::Endpoint,
    io::Io,
    simplex::{client::connect as simplex_connect, Config},
    Result,
};
use futures::Future;

pub async fn connect<F: Future<Output = Result<impl Io>>, C: FnOnce(&Endpoint) -> F>(
    connector: C,
    endpoint: &Endpoint,
    next_hop: &Endpoint,
    config: &Config,
) -> Result<impl Io> {
    let s = simplex_connect(
        connector(next_hop).await?,
        endpoint,
        config,
        next_hop.to_string(),
    )
    .await?;

    Ok(s)
}
