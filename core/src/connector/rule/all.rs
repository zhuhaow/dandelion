use super::Rule;
use crate::{connector::BoxedConnector, endpoint::Endpoint};

pub struct AllRule {
    connector: BoxedConnector,
}

impl AllRule {
    pub fn new(connector: BoxedConnector) -> Self {
        Self { connector }
    }
}

#[async_trait::async_trait]
impl Rule for AllRule {
    async fn check(&self, _endpoint: &Endpoint) -> Option<&BoxedConnector> {
        Some(&self.connector)
    }
}
