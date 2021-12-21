use super::Rule;
use crate::{connector::BoxedConnector, endpoint::Endpoint};
use regex::Regex;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub enum Mode {
    Prefix(String),
    Suffix(String),
    Keyword(String),
    Regex(#[serde(with = "serde_regex")] Regex),
}

pub struct DomainRule {
    modes: Vec<Mode>,
    connector: BoxedConnector,
}

impl DomainRule {
    pub fn new(modes: Vec<Mode>, connector: BoxedConnector) -> Self {
        Self { modes, connector }
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
                    return Some(self.connector.clone());
                }
            }
        }
        None
    }
}
