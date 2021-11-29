pub mod tcp;

use crate::{io::Io, Endpoint, Result};
use std::marker::PhantomData;

#[async_trait::async_trait]
pub trait Connector<T: Io> {
    async fn connect(self, endpoint: &Endpoint) -> Result<T>;
}

pub struct ConnectorWrapper<T: Io, Conn: Connector<T> + Send> {
    connector: Conn,
    _marker: PhantomData<T>,
}

impl<T: Io, Conn: Connector<T> + Send> ConnectorWrapper<T, Conn> {
    pub fn new(connector: Conn) -> Self {
        Self {
            connector,
            _marker: PhantomData::default(),
        }
    }
}

#[async_trait::async_trait]
impl<T: Io, Conn: Connector<T> + Send> Connector<Box<dyn Io>> for ConnectorWrapper<T, Conn> {
    async fn connect(self, endpoint: &Endpoint) -> Result<Box<dyn Io>> {
        let stream = self.connector.connect(endpoint).await?;
        Ok(Box::new(stream))
    }
}
