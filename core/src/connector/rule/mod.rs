pub mod all;
pub mod dns_fail;
pub mod domain;
pub mod geoip;
pub mod ip;

use super::Connector;
use crate::{endpoint::Endpoint, Result};
use anyhow::anyhow;
use std::marker::PhantomData;

pub struct RuleConnector<C: Connector> {
    rules: Vec<Box<dyn Rule<C>>>,
    _marker: PhantomData<C>,
}

impl<C: Connector> RuleConnector<C> {
    pub fn new(rules: Vec<Box<dyn Rule<C>>>) -> Self {
        Self {
            rules,
            _marker: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Connector for RuleConnector<C> {
    type Stream = C::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        for rule in self.rules.iter() {
            match rule.check(endpoint).await {
                Some(c) => return c.connect(endpoint).await,
                None => continue,
            }
        }

        return Err(anyhow!("No rule match the target endpoint"));
    }
}

#[async_trait::async_trait]
pub trait Rule<C: Connector>: Sync + Send {
    async fn check(&self, endpoint: &Endpoint) -> Option<&C>;
}
