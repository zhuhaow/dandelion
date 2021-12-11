use super::Rule;
use crate::{
    connector::{
        boxed::{BoxedConnector, BoxedConnectorFactory},
        ConnectorFactory,
    },
    endpoint::Endpoint,
};
use regex::Regex;

pub enum Mode {
    Prefix(String),
    Suffix(String),
    Keyword(String),
    Regex(Regex),
}

pub struct DomainRule {
    modes: Vec<Mode>,
    factory: BoxedConnectorFactory,
}

impl DomainRule {
    pub fn new(modes: Vec<Mode>, factory: BoxedConnectorFactory) -> Self {
        Self { modes, factory }
    }
}

#[async_trait::async_trait]
impl Rule for DomainRule {
    async fn check(&self, endpoint: &Endpoint) -> Option<BoxedConnector> {
        if let Endpoint::Domain(d, _) = endpoint {
            for mode in self.modes.iter() {
                if match mode {
                    Mode::Prefix(p) => d.starts_with(p),
                    Mode::Suffix(s) => d.ends_with(s),
                    Mode::Keyword(k) => d.contains(k),
                    Mode::Regex(r) => r.is_match(d),
                } {
                    return Some(self.factory.build());
                }
            }
        }
        None
    }
}
