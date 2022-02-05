use super::Resolver;
use crate::Result;
use itertools::Itertools;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    str::FromStr,
    time::Duration,
};
use tokio::{net::UdpSocket, time::timeout};
use trust_dns_client::{
    client::{AsyncClient, ClientHandle},
    op::Message,
    rr::{DNSClass, Name, RecordType},
    udp::UdpClientStream,
};
use trust_dns_proto::{
    xfer::{DnsRequest, DnsRequestOptions},
    DnsHandle,
};

pub struct UdpResolver {
    client: AsyncClient,
    timeout: Duration,
}

impl UdpResolver {
    pub async fn new(addr: SocketAddr, timeout: Duration) -> Result<Self> {
        let stream = UdpClientStream::<UdpSocket>::new(addr);
        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        Ok(Self { client, timeout })
    }
}

#[async_trait::async_trait]
impl Resolver for UdpResolver {
    async fn lookup_ip(&self, name: &str) -> Result<Vec<IpAddr>> {
        Ok(timeout(self.timeout, self.lookup_ipv4(name))
            .await??
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn lookup_ipv4(&self, name: &str) -> Result<Vec<Ipv4Addr>> {
        Ok(timeout(
            self.timeout,
            self.client
                .clone()
                .query(Name::from_str(name)?, DNSClass::IN, RecordType::A),
        )
        .await??
        .take_answers()
        .into_iter()
        .filter_map(|r| match r.into_data() {
            trust_dns_client::rr::RData::A(ip) => Some(ip),
            _ => None,
        })
        .collect_vec())
    }

    async fn lookup_ipv6(&self, name: &str) -> Result<Vec<Ipv6Addr>> {
        Ok(timeout(
            self.timeout,
            self.client
                .clone()
                .query(Name::from_str(name)?, DNSClass::IN, RecordType::AAAA),
        )
        .await??
        .take_answers()
        .into_iter()
        .filter_map(|r| match r.into_data() {
            trust_dns_client::rr::RData::AAAA(ip) => Some(ip),
            _ => None,
        })
        .collect_vec())
    }

    async fn lookup_raw(&self, message: Message) -> Result<Message> {
        // Here is a little tricky, the implementation internally would randomly
        // set a id so we need to restore the id.
        let id = message.id();

        let mut response = timeout(
            self.timeout,
            self.client
                .clone()
                .send(DnsRequest::new(message, DnsRequestOptions::default())),
        )
        .await??;

        response.set_id(id);

        Ok(response.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_dns_client::op::{MessageType, OpCode, Query};

    #[tokio::test]
    async fn resolve() -> Result<()> {
        let resolver =
            UdpResolver::new("8.8.8.8:53".parse().unwrap(), Duration::from_secs(5)).await?;

        assert!(!resolver.lookup_ip("apple.com").await?.is_empty());
        assert!(!resolver.lookup_ipv4("apple.com").await?.is_empty());
        assert!(!resolver.lookup_ipv6("facebook.com").await?.is_empty());

        let mut message = Message::new();
        message.set_op_code(OpCode::Query);
        message.set_message_type(MessageType::Query);
        let query = Query::query(Name::from_str("apple.com").unwrap(), RecordType::A);
        message.add_query(query);
        assert!(resolver.lookup_raw(message).await?.answer_count() > 0);

        let mut message = Message::new();
        message.set_op_code(OpCode::Query);
        message.set_message_type(MessageType::Query);
        let query = Query::query(Name::from_str("gmail.com").unwrap(), RecordType::MX);
        message.add_query(query);
        assert!(resolver.lookup_raw(message).await?.answer_count() > 0);

        Ok(())
    }
}
