use super::{Acceptor, MidHandshake};
pub use crate::simplex::server::SimplexMidHandshake;
use crate::{
    io::Io,
    simplex::{server::serve, Config},
    Result,
};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct SimplexAcceptor<I: Io> {
    config: Config,
    _marker: PhantomData<I>,
}

impl<I: Io> SimplexAcceptor<I> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            _marker: Default::default(),
        }
    }
}

impl<I: Io> Clone for SimplexAcceptor<I> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            _marker: self._marker,
        }
    }
}

#[async_trait::async_trait]
impl<I: Io> Acceptor for SimplexAcceptor<I> {
    type Input = I;
    type Output = SimplexMidHandshake;

    async fn handshake(self, io: I) -> Result<Self::Output> {
        serve(io, self.config).await
    }
}

#[async_trait::async_trait]
impl MidHandshake for SimplexMidHandshake {
    type Output = Box<dyn Io>;

    fn target_endpoint(&self) -> &crate::endpoint::Endpoint {
        self.taget_endpoint()
    }

    async fn finalize(self) -> Result<Self::Output> {
        self.finalize().await
    }
}
