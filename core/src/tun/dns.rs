use crate::{utils::expiring_hash::ExpiringHashMap, Result};
use anyhow::bail;
use ipnetwork::IpNetworkIterator;
use std::{
    collections::LinkedList,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{net::UdpSocket, sync::Mutex};
use trust_dns_client::{
    client::AsyncClient,
    op::{Message, MessageType},
    rr::{RData, Record},
    udp::UdpClientStream,
};
use trust_dns_proto::{
    xfer::{DnsRequest, DnsRequestOptions},
    DnsHandle,
};

// Only IPv4 is supported for now.
// TODO: Add IPv6 support.
pub struct FakeDns {
    sender: AsyncClient,
    pool: Arc<Mutex<DnsFakeIpPool>>,
}

impl FakeDns {
    pub async fn new(server: SocketAddr, ip_range: IpNetworkIterator) -> Result<Self> {
        let stream = UdpClientStream::<UdpSocket>::new(server);
        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        Ok(Self {
            sender: client,
            pool: Arc::new(Mutex::new(DnsFakeIpPool::new(ip_range))),
        })
    }

    pub async fn handle(&self, request: Message) -> Result<Message> {
        let request_domain = request.queries().iter().find_map(|q| match q.query_type() {
            trust_dns_proto::rr::RecordType::A => Some(q.name()),
            _ => None,
        });

        if let Some(domain) = request_domain {
            let domain_str = domain.to_utf8();
            if self.should_use_fake_ip(domain_str.as_str()) {
                let mut pool = self.pool.lock().await;
                let ip = pool.get_fake_ip(domain_str)?;

                let mut response = request.clone();
                response.set_message_type(MessageType::Response);

                let rdata = match ip {
                    IpAddr::V4(ip) => RData::A(ip),
                    IpAddr::V6(ip) => RData::AAAA(ip),
                };

                response.add_answer(Record::from_rdata(domain.clone(), pool.ttl() as u32, rdata));

                return Ok(response);
            }
        }

        Ok(self
            .sender
            .clone()
            .send(DnsRequest::new(request, DnsRequestOptions::default()))
            .await?
            .into())
    }

    // TODO: We should support a suffix to query real IP address. E.g.,
    // google.com.test -> google.com
    fn should_use_fake_ip(&self, _domain: &str) -> bool {
        true
    }

    async fn _reverse_lookup(&self, addr: &IpAddr) -> Option<String> {
        self.pool.lock().await.map.get(addr).map(String::clone)
    }
}

struct DnsFakeIpPool {
    fake_ip_pool: LinkedList<IpAddr>,
    ip_iter: IpNetworkIterator,
    map: ExpiringHashMap<IpAddr, String>,
}

impl DnsFakeIpPool {
    fn new(ip_range: IpNetworkIterator) -> Self {
        Self {
            fake_ip_pool: Default::default(),
            ip_iter: ip_range,
            map: ExpiringHashMap::new(Duration::from_secs(15), true),
        }
    }

    fn ttl(&self) -> u64 {
        self.map.get_ttl().as_secs().saturating_sub(5)
    }

    fn get_fake_ip(&mut self, domain: String) -> Result<IpAddr> {
        let ip = match self.fake_ip_pool.pop_front() {
            Some(ip) => ip,
            None => {
                self.fill_pool();
                match self.fake_ip_pool.pop_front() {
                    Some(ip) => ip,
                    None => bail!("Failed to create fake ip, the pool is drained"),
                }
            }
        };

        self.map.insert(ip, domain);

        Ok(ip)
    }

    fn fill_pool(&mut self) {
        self.clear_outdated_map();
        if self.fake_ip_pool.is_empty() {
            self.fake_ip_pool.extend(self.ip_iter.by_ref().take(10));
        }
    }

    fn clear_outdated_map(&mut self) {
        let released = self.map.evict_expired();

        self.fake_ip_pool.extend(released.into_iter().map(|v| v.0));
    }
}
