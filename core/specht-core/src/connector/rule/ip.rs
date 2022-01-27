use super::Rule;
use crate::{connector::Connector, endpoint::Endpoint, resolver::Resolver};
use ipnetwork::IpNetwork;

pub struct IpRule<R: Resolver, C: Connector> {
    subnets: Vec<IpNetwork>,
    connector: C,
    resolver: R,
}

impl<R: Resolver, C: Connector> IpRule<R, C> {
    pub fn new(subnets: Vec<IpNetwork>, connector: C, resolver: R) -> Self {
        Self {
            subnets,
            connector,
            resolver,
        }
    }
}

#[async_trait::async_trait]
impl<R: Resolver, C: Connector> Rule<C> for IpRule<R, C> {
    async fn check(&self, endpoint: &Endpoint) -> Option<&C> {
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
