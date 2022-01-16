use super::Rule;
use crate::{connector::BoxedConnector, endpoint::Endpoint, resolver::Resolver};
use ipnetwork::IpNetwork;

pub struct IpRule<R: Resolver> {
    subnets: Vec<IpNetwork>,
    connector: BoxedConnector,
    resolver: R,
}

impl<R: Resolver> IpRule<R> {
    pub fn new(subnets: Vec<IpNetwork>, connector: BoxedConnector, resolver: R) -> Self {
        Self {
            subnets,
            connector,
            resolver,
        }
    }
}

#[async_trait::async_trait]
impl<R: Resolver> Rule for IpRule<R> {
    async fn check(&self, endpoint: &Endpoint) -> Option<&BoxedConnector> {
        match endpoint {
            Endpoint::Addr(addr) => {
                for network in self.subnets.iter() {
                    if network.contains(addr.ip()) {
                        return Some(&self.connector);
                    }
                }
            }
            Endpoint::Domain(host, _) => {
                let ips = self.resolver.lookup_ip(host.as_str()).await.ok()?;
                for ip in ips {
                    for network in self.subnets.iter() {
                        if network.contains(ip) {
                            return Some(&self.connector);
                        }
                    }
                }
            }
        };

        None
    }
}
