pub mod simplex;
pub mod tcp;

use crate::{endpoint::Endpoint, io::Io, Result};

#[async_trait::async_trait]
pub trait Connector: Clone + Sync + Send + 'static {
    type Stream: Io;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream>;
}
