use super::Rule;
use crate::{connector::BoxedConnector, endpoint::Endpoint};
use ipnetwork::IpNetwork;
use tokio::net::lookup_host;

pub struct IpRule {
    subnets: Vec<IpNetwork>,
    connector: BoxedConnector,
}

impl IpRule {
    pub fn new(subnets: Vec<IpNetwork>, connector: BoxedConnector) -> Self {
        Self { subnets, connector }
    }
}

#[async_trait::async_trait]
impl Rule for IpRule {
    async fn check(&self, endpoint: &Endpoint) -> Option<&BoxedConnector> {
        match endpoint {
            Endpoint::Addr(addr) => {
                for network in self.subnets.iter() {
                    if network.contains(addr.ip()) {
                        return Some(&self.connector);
                    }
                }
            }
            Endpoint::Domain(host, port) => {
                let addrs = lookup_host((host.as_str(), *port)).await.ok()?;
                for addr in addrs {
                    for network in self.subnets.iter() {
                        if network.contains(addr.ip()) {
                            return Some(&self.connector);
                        }
                    }
                }
            }
        };

        None
    }
}
