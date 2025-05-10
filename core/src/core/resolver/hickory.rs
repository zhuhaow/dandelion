use super::Resolver;
use crate::Result;
use anyhow::bail;
use hickory_proto::op::{Message, MessageType};
use hickory_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    name_server::TokioConnectionProvider,
    TokioResolver,
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

#[derive(Debug)]
pub struct HickoryResolver {
    client: TokioResolver,
}

impl HickoryResolver {
    pub fn new(nameservers: Vec<NameServerConfig>, timeout: Duration) -> Result<Self> {
        let mut options = ResolverOpts::default();
        options.timeout = timeout;

        let mut config = ResolverConfig::default();
        for nameserver in nameservers {
            config.add_name_server(nameserver);
        }

        Ok(Self {
            client: TokioResolver::builder_with_config(config, TokioConnectionProvider::default())
                .with_options(options)
                .build(),
        })
    }
}

#[async_trait::async_trait]
impl Resolver for HickoryResolver {
    async fn lookup_ip(&self, name: &str) -> Result<Vec<IpAddr>> {
        Ok(self.client.lookup_ip(name).await?.into_iter().collect()).and_then(|r: Vec<IpAddr>| {
            if r.is_empty() {
                bail!("Failed to find result for domain {}", name)
            } else {
                Ok(r)
            }
        })
    }

    async fn lookup_ipv4(&self, name: &str) -> Result<Vec<Ipv4Addr>> {
        Ok(self
            .client
            .ipv4_lookup(name)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
        .and_then(|r: Vec<Ipv4Addr>| {
            if r.is_empty() {
                bail!("Failed to find result for domain {}", name)
            } else {
                Ok(r)
            }
        })
    }

    async fn lookup_ipv6(&self, name: &str) -> Result<Vec<Ipv6Addr>> {
        Ok(self
            .client
            .ipv6_lookup(name)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
        .and_then(|r: Vec<Ipv6Addr>| {
            if r.is_empty() {
                bail!("Failed to find result for domain {}", name)
            } else {
                Ok(r)
            }
        })
    }

    async fn lookup_raw(&self, mut message: Message) -> Result<Message> {
        let query = message
            .queries()
            .first()
            .to_owned()
            .ok_or_else(|| anyhow::anyhow!("Receive DNS request with no query item"))?;

        let result = self
            .client
            .lookup(query.name().clone(), query.query_type())
            .await?;

        message
            .add_answers(result.record_iter().cloned())
            .set_message_type(MessageType::Response);

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::xfer::Protocol;
    use hickory_proto::{
        op::{MessageType, OpCode, Query},
        rr::RecordType,
    };
    use hickory_resolver::Name;
    use std::str::FromStr;

    #[tokio::test]
    async fn resolve() -> Result<()> {
        let resolver = HickoryResolver::new(
            vec![NameServerConfig {
                socket_addr: "8.8.8.8:53".parse().unwrap(),
                protocol: Protocol::Udp,
                tls_dns_name: None,
                http_endpoint: None,
                trust_negative_responses: true,
                bind_addr: None,
            }],
            Duration::from_secs(5),
        )?;

        assert!(!resolver.lookup_ip("apple.com").await?.is_empty());
        assert!(!resolver.lookup_ipv4("apple.com").await?.is_empty());
        assert!(!resolver.lookup_ipv6("facebook.com").await?.is_empty());

        let mut message = Message::new();
        message.set_op_code(OpCode::Query);
        message.set_message_type(MessageType::Query);
        let query = Query::query(Name::from_str("apple.com").unwrap(), RecordType::A);
        message.add_query(query);
        assert!(!resolver.lookup_raw(message).await?.answers().is_empty());

        let mut message = Message::new();
        message.set_op_code(OpCode::Query);
        message.set_message_type(MessageType::Query);
        let query = Query::query(Name::from_str("gmail.com").unwrap(), RecordType::MX);
        message.add_query(query);
        assert!(!resolver.lookup_raw(message).await?.answers().is_empty());

        Ok(())
    }
}
