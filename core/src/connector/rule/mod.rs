pub mod all;
pub mod dns_fail;
pub mod domain;
pub mod geoip;
pub mod ip;

use super::{boxed::BoxedConnector, Connector, ConnectorFactory};
use crate::{endpoint::Endpoint, Result};
use anyhow::anyhow;
use std::sync::Arc;

pub struct RuleConnector {
    rules: Arc<Vec<Box<dyn Rule>>>,
}

#[async_trait::async_trait]
impl Connector for RuleConnector {
    type Stream = <BoxedConnector as Connector>::Stream;

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

pub struct RuleConnectorFactory {
    rules: Arc<Vec<Box<dyn Rule>>>,
}

impl RuleConnectorFactory {
    pub fn new(rules: Arc<Vec<Box<dyn Rule>>>) -> Self {
        Self { rules }
    }
}

impl ConnectorFactory for RuleConnectorFactory {
    type Product = BoxedConnector;

    fn build(&self) -> Self::Product {
        BoxedConnector::new(RuleConnector {
            rules: self.rules.clone(),
        })
    }
}

#[async_trait::async_trait]
pub trait Rule: Sync + Send {
    async fn check(&self, endpoint: &Endpoint) -> Option<BoxedConnector>;
}
