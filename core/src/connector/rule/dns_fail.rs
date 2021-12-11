use super::Rule;
use crate::{
    connector::{
        boxed::{BoxedConnector, BoxedConnectorFactory},
        ConnectorFactory,
    },
    endpoint::Endpoint,
};
use tokio::net::lookup_host;

pub struct DnsFailRule {
    factory: BoxedConnectorFactory,
}

impl DnsFailRule {
    pub fn new(factory: BoxedConnectorFactory) -> Self {
        Self { factory }
    }
}

#[async_trait::async_trait]
impl Rule for DnsFailRule {
    async fn check(&self, endpoint: &Endpoint) -> Option<BoxedConnector> {
        if let Endpoint::Domain(host, port) = endpoint {
            let result = lookup_host((host.as_str(), *port)).await;
            match result {
                Ok(addrs) => {
                    if addrs.count() == 0 {
                        return Some(self.factory.build());
                    }
                }
                Err(_) => return Some(self.factory.build()),
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        connector::{boxed::BoxedConnectorFactory, rule::Rule, tcp::TcpConnectorFactory},
        endpoint::Endpoint,
    };
    use rstest::*;

    use super::DnsFailRule;

    #[rstest]
    #[case("t.test", true)]
    #[case("t.invalid", true)]
    #[case("google.com", false)]
    #[tokio::test]
    async fn test_dns_fail(#[case] domain: &str, #[case] is_some: bool) {
        let factory = BoxedConnectorFactory::new(TcpConnectorFactory::default());
        let rule = DnsFailRule::new(factory);

        assert_eq!(
            rule.check(&Endpoint::Domain(domain.to_owned(), 443))
                .await
                .is_some(),
            is_some
        );
    }
}
