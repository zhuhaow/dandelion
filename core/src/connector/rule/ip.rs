use super::Rule;
use crate::{
    connector::{
        boxed::{BoxedConnector, BoxedConnectorFactory},
        ConnectorFactory,
    },
    endpoint::Endpoint,
};
use ipnetwork::IpNetwork;
use tokio::net::lookup_host;

pub struct IpRule {
    subnets: Vec<IpNetwork>,
    factory: BoxedConnectorFactory,
}

impl IpRule {
    pub fn new(subnets: Vec<IpNetwork>, factory: BoxedConnectorFactory) -> Self {
        Self { subnets, factory }
    }
}

#[async_trait::async_trait]
impl Rule for IpRule {
    async fn check(&self, endpoint: &Endpoint) -> Option<BoxedConnector> {
        match endpoint {
            Endpoint::Addr(addr) => {
                for network in self.subnets.iter() {
                    if network.contains(addr.ip()) {
                        return Some(self.factory.build());
                    }
                }
            }
            Endpoint::Domain(host, port) => {
                let addrs = lookup_host((host.as_str(), *port)).await.ok()?;
                for addr in addrs {
                    for network in self.subnets.iter() {
                        if network.contains(addr.ip()) {
                            return Some(self.factory.build());
                        }
                    }
                }
            }
        };

        None
    }
}
