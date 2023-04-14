use super::Rule;
use crate::{connector::Connector, endpoint::Endpoint, resolver::Resolver};
use tracing::debug;

pub struct DnsFailRule<R: Resolver, C: Connector> {
    connector: C,
    resolver: R,
}

impl<R: Resolver, C: Connector> DnsFailRule<R, C> {
    pub fn new(connector: C, resolver: R) -> Self {
        Self {
            connector,
            resolver,
        }
    }
}

#[async_trait::async_trait]
impl<R: Resolver, C: Connector> Rule<C> for DnsFailRule<R, C> {
    async fn check(&self, endpoint: &Endpoint) -> Option<&C> {
        if let Endpoint::Domain(host, _) = endpoint {
            let result = self.resolver.lookup_ip(host.as_str()).await;
            match result {
                Ok(ips) => {
                    if ips.is_empty() {
                        debug!(
                            "Matched since resolve failed, domain {} is resolved with no result",
                            host
                        );
                        return Some(&self.connector);
                    }
                }
                Err(err) => {
                    debug!(
                        "Matched since resolved failed for domain {} due to {}",
                        host, err
                    );

                    return Some(&self.connector);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::DnsFailRule;
    use crate::{
        connector::{rule::Rule, tcp::TcpConnector},
        endpoint::Endpoint,
        resolver::system::SystemResolver,
    };
    use rstest::*;

    #[rstest]
    #[case("t.test", true)]
    #[case("t.invalid", true)]
    #[case("google.com", false)]
    #[tokio::test]
    async fn test_dns_fail(#[case] domain: &str, #[case] is_some: bool) {
        let rule = DnsFailRule::new(
            TcpConnector::new(SystemResolver::new()),
            SystemResolver::new(),
        );

        assert_eq!(
            rule.check(&Endpoint::Domain(domain.to_owned(), 443))
                .await
                .is_some(),
            is_some
        );
    }
}
