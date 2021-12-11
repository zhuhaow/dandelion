use super::Rule;
use crate::{
    connector::{
        boxed::{BoxedConnector, BoxedConnectorFactory},
        ConnectorFactory,
    },
    endpoint::Endpoint,
};

pub struct AllRule {
    factory: BoxedConnectorFactory,
}

impl AllRule {
    pub fn new(factory: BoxedConnectorFactory) -> Self {
        Self { factory }
    }
}

#[async_trait::async_trait]
impl Rule for AllRule {
    async fn check(&self, _endpoint: &Endpoint) -> Option<BoxedConnector> {
        Some(self.factory.build())
    }
}
