use super::Rule;
use crate::{connector::Connector, endpoint::Endpoint};
use regex::Regex;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize, Clone)]
pub enum Mode {
    Prefix(String),
    Suffix(String),
    Keyword(String),
    Regex(#[serde(with = "serde_regex")] Regex),
}

pub struct DomainRule<C: Connector> {
    modes: Vec<Mode>,
    connector: C,
}

impl<C: Connector> DomainRule<C> {
    pub fn new(modes: Vec<Mode>, connector: C) -> Self {
        Self { modes, connector }
    }
}

#[async_trait::async_trait]
impl<C: Connector> Rule<C> for DomainRule<C> {
    async fn check(&self, endpoint: &Endpoint) -> Option<&C> {
        if let Endpoint::Domain(d, _) = endpoint {
            debug!("Matching endpoint with domain {}", d);
            // The domain should be FQDN but may come with two forms, w/ or w/o
            // the ending dot. We don't want the user to deal with that.
            let d = d
                .strip_suffix('.')
                .map(|d| {
                    debug!(
                        "Removing the ending dot from domain, matching with new domain {}",
                        d
                    );
                    d
                })
                .unwrap_or(d);
            for mode in self.modes.iter() {
                if match mode {
                    Mode::Prefix(p) => d.starts_with(p),
                    Mode::Suffix(s) => d.ends_with(s),
                    Mode::Keyword(k) => d.contains(k),
                    Mode::Regex(r) => r.is_match(d),
                } {
                    debug!("Matched domain {} with mode {:#?}", d, mode);
                    return Some(&self.connector);
                }
            }
        }
        None
    }
}
