use super::Rule;
use crate::{connector::BoxedConnector, endpoint::Endpoint, resolver::Resolver};

pub struct DnsFailRule<R: Resolver> {
    connector: BoxedConnector,
    resolver: R,
}

impl<R: Resolver> DnsFailRule<R> {
    pub fn new(connector: BoxedConnector, resolver: R) -> Self {
        Self {
            connector,
            resolver,
        }
    }
}

#[async_trait::async_trait]
impl<R: Resolver> Rule for DnsFailRule<R> {
    async fn check(&self, endpoint: &Endpoint) -> Option<&BoxedConnector> {
        if let Endpoint::Domain(host, _) = endpoint {
            let result = self.resolver.lookup_ip(host.as_str()).await;
            match result {
                Ok(ips) => {
                    if ips.is_empty() {
                        return Some(&self.connector);
                    }
                }
                Err(_) => return Some(&self.connector),
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::DnsFailRule;
    use crate::{
        connector::{rule::Rule, tcp::TcpConnector, Connector},
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
            TcpConnector::new(SystemResolver::new()).boxed(),
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
