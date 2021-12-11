use super::{Acceptor, HandshakeResult};
pub use crate::simplex::server::handshake;
use crate::{io::Io, simplex::Config};
use futures::FutureExt;

#[derive(Debug, Clone)]
pub struct SimplexAcceptor {
    config: Config,
}

impl SimplexAcceptor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl<I: Io> Acceptor<I> for SimplexAcceptor {
    async fn do_handshake(&self, io: I) -> HandshakeResult {
        let (endpoint, fut) = handshake(io, self.config.clone()).await?;
        Ok((
            endpoint,
            async move {
                let io: Box<dyn Io> = Box::new(fut.await?);
                Ok(io)
            }
            .boxed(),
        ))
    }
}
