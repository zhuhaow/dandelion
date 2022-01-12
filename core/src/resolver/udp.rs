use super::Resolver;
use crate::Result;
use itertools::Itertools;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    str::FromStr,
};
use tokio::net::UdpSocket;
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
}

impl UdpResolver {
    pub async fn new(addr: SocketAddr) -> Result<Self> {
        let stream = UdpClientStream::<UdpSocket>::new(addr);
        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl Resolver for UdpResolver {
    async fn lookup_ip(&self, name: &str) -> Result<Vec<IpAddr>> {
        Ok(self
            .lookup_ipv4(name)
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn lookup_ipv4(&self, name: &str) -> Result<Vec<Ipv4Addr>> {
        Ok(self
            .client
            .clone()
            .query(Name::from_str(name)?, DNSClass::IN, RecordType::A)
            .await?
            .take_answers()
            .into_iter()
            .filter_map(|r| match r.into_data() {
                trust_dns_client::rr::RData::A(ip) => Some(ip),
                _ => None,
            })
            .collect_vec())
    }

    async fn lookup_ipv6(&self, name: &str) -> Result<Vec<Ipv6Addr>> {
        Ok(self
            .client
            .clone()
            .query(Name::from_str(name)?, DNSClass::IN, RecordType::AAAA)
            .await?
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

        let mut response = self
            .client
            .clone()
            .send(DnsRequest::new(message, DnsRequestOptions::default()))
            .await?;

        response.set_id(id);

        Ok(response.into())
    }
}
