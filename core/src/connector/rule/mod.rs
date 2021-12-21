pub mod all;
pub mod dns_fail;
pub mod domain;
pub mod geoip;
pub mod ip;

use super::{BoxedConnector, Connector};
use crate::{endpoint::Endpoint, io::Io, Result};
use anyhow::anyhow;
use std::sync::Arc;

#[derive(Clone)]
pub struct RuleConnector {
    rules: Arc<Vec<Box<dyn Rule>>>,
}

impl RuleConnector {
    pub fn new(rules: Vec<Box<dyn Rule>>) -> Self {
        Self {
            rules: Arc::new(rules),
        }
    }
}

#[async_trait::async_trait]
impl Connector for RuleConnector {
    type Stream = Box<dyn Io>;

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
pub trait Rule: Sync + Send {
    async fn check(&self, endpoint: &Endpoint) -> Option<BoxedConnector>;
}
