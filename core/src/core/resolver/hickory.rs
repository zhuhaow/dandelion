use super::Resolver;
use crate::Result;
use anyhow::bail;
use hickory_proto::{
    op::{Message, MessageType},
    rr::RData,
};
use hickory_resolver::{
    config::{ConnectionConfig, NameServerConfig, ResolverConfig, ResolverOpts, GOOGLE},
    net::runtime::TokioRuntimeProvider,
    TokioResolver,
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

#[derive(Debug)]
pub struct HickoryResolver {
    client: TokioResolver,
}

impl HickoryResolver {
    pub fn new(nameservers: Vec<NameServerConfig>, timeout: Duration) -> Result<Self> {
        // Keep the upstreams that Hickory 0.25 included in its default configuration.
        let mut config = ResolverConfig::udp_and_tcp(&GOOGLE);
        config.name_servers.extend(nameservers);
        Self::with_config(config, timeout)
    }

    fn with_config(config: ResolverConfig, timeout: Duration) -> Result<Self> {
        let mut options = ResolverOpts::default();
        options.timeout = timeout;
        options.edns0 = false;

        Ok(Self {
            client: TokioResolver::builder_with_config(config, TokioRuntimeProvider::default())
                .with_options(options)
                .build()?,
        })
    }

    pub(crate) fn new_udp(addrs: Vec<SocketAddr>, timeout: Duration) -> Result<Self> {
        let nameservers = addrs
            .into_iter()
            .map(udp_nameserver)
            .collect::<Result<_>>()?;
        Self::new(nameservers, timeout)
    }
}

fn udp_nameserver(addr: SocketAddr) -> Result<NameServerConfig> {
    if matches!(addr, SocketAddr::V6(addr) if addr.scope_id() != 0) {
        bail!("Hickory DNS does not support IPv6 scope IDs in upstream address {addr}");
    }
    let mut connection = ConnectionConfig::udp();
    connection.port = addr.port();
    Ok(NameServerConfig::new(addr.ip(), true, vec![connection]))
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
            .answers()
            .iter()
            .filter_map(|record| match &record.data {
                RData::A(addr) => Some(addr.0),
                _ => None,
            })
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
            .answers()
            .iter()
            .filter_map(|record| match &record.data {
                RData::AAAA(addr) => Some(addr.0),
                _ => None,
            })
            .collect())
        .and_then(|r: Vec<Ipv6Addr>| {
            if r.is_empty() {
                bail!("Failed to find result for domain {}", name)
            } else {
                Ok(r)
            }
        })
    }

    fn support_raw(&self) -> bool {
        true
    }

    async fn lookup_raw(&self, mut message: Message) -> Result<Message> {
        let query = message
            .queries
            .first()
            .ok_or_else(|| anyhow::anyhow!("Receive DNS request with no query item"))?;

        let result = self
            .client
            .lookup(query.name().clone(), query.query_type())
            .await?;

        message.add_answers(result.answers().iter().cloned());
        message.metadata.message_type = MessageType::Response;

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::{
        op::Query,
        rr::{
            rdata::{A, AAAA, MX},
            Name, Record, RecordType,
        },
    };
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn resolve() -> Result<()> {
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        let config = ResolverConfig::from_name_servers(vec![udp_nameserver(socket.local_addr()?)?]);
        let resolver = HickoryResolver::with_config(config, Duration::from_secs(2))?;
        let name = "example.test.";
        let ipv4 = Ipv4Addr::new(192, 0, 2, 1);
        let ipv6 = "2001:db8::1".parse::<Ipv6Addr>()?;
        let mx = RData::MX(MX::new(10, Name::from_ascii("mail.example.test.")?));

        let serve = async {
            let mut buf = [0; 512];
            loop {
                let (len, peer) = socket.recv_from(&mut buf).await?;
                let request = Message::from_vec(&buf[..len])?;
                let query = &request.queries[0];
                let data = match query.query_type() {
                    RecordType::A => RData::A(A(ipv4)),
                    RecordType::AAAA => RData::AAAA(AAAA(ipv6)),
                    RecordType::MX => mx.clone(),
                    other => bail!("Unexpected DNS query type {other}"),
                };
                let mut response = Message::response(request.metadata.id, request.metadata.op_code);
                response.add_query(query.clone());
                response.add_answer(Record::from_rdata(query.name().clone(), 60, data));
                socket.send_to(&response.to_vec()?, peer).await?;
            }
        };

        let lookup = async {
            assert!(!resolver.lookup_ip(name).await?.is_empty());
            assert_eq!(resolver.lookup_ipv4(name).await?, [ipv4]);
            assert_eq!(resolver.lookup_ipv6(name).await?, [ipv6]);

            let mut message = Message::query();
            message.add_query(Query::query(Name::from_ascii(name)?, RecordType::MX));
            let response = resolver.lookup_raw(message).await?;
            assert_eq!(response.metadata.message_type, MessageType::Response);
            assert_eq!(response.answers[0].data, mx);
            Ok(())
        };

        tokio::select! {
            result = serve => result,
            result = lookup => result,
        }
    }
}
