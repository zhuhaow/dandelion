use crate::{
    core::{
        endpoint::Endpoint,
        io::Io,
        simplex::{client::connect as simplex_connect, Config},
    },
    Result,
};

pub async fn connect(endpoint: &Endpoint, config: &Config, nexthop: impl Io) -> Result<impl Io> {
    let s = simplex_connect(nexthop, endpoint, config).await?;

    Ok(s)
}
