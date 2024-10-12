use crate::{core::endpoint::Endpoint, Result};
use anyhow::bail;
use futures::never::Never;

pub async fn connect(endpoint: &Endpoint) -> Result<Never> {
    bail!("Connection to {} is blocked", endpoint);
}
