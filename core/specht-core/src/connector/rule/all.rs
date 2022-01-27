use super::Rule;
use crate::{connector::Connector, endpoint::Endpoint};

pub struct AllRule<C: Connector> {
    connector: C,
}

impl<C: Connector> AllRule<C> {
    pub fn new(connector: C) -> Self {
        Self { connector }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Rule<C> for AllRule<C> {
    async fn check(&self, _endpoint: &Endpoint) -> Option<&C> {
        Some(&self.connector)
    }
}
