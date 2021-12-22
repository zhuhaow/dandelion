use super::Rule;
use crate::{connector::BoxedConnector, endpoint::Endpoint};
use tokio::net::lookup_host;

pub struct DnsFailRule {
    connector: BoxedConnector,
}

impl DnsFailRule {
    pub fn new(connector: BoxedConnector) -> Self {
        Self { connector }
    }
}

#[async_trait::async_trait]
impl Rule for DnsFailRule {
    async fn check(&self, endpoint: &Endpoint) -> Option<&BoxedConnector> {
        if let Endpoint::Domain(host, port) = endpoint {
            let result = lookup_host((host.as_str(), *port)).await;
            match result {
                Ok(addrs) => {
                    if addrs.count() == 0 {
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
        let rule = DnsFailRule::new(TcpConnector::new(SystemResolver::new()).boxed());

        assert_eq!(
            rule.check(&Endpoint::Domain(domain.to_owned(), 443))
                .await
                .is_some(),
            is_some
        );
    }
}
