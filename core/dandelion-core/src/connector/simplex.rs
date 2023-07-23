use crate::{
    endpoint::Endpoint,
    io::Io,
    simplex::{client::connect as simplex_connect, Config},
    Result,
};

pub async fn connect(
    endpoint: &Endpoint,
    host: &str,
    config: &Config,
    nexthop: impl Io,
) -> Result<impl Io> {
    let s = simplex_connect(nexthop, endpoint, config, host.to_owned()).await?;

    Ok(s)
}
