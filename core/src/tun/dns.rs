use crate::{utils::expiring_hash::ExpiringHashMap, Result};
use anyhow::bail;
use ipnetwork::Ipv4NetworkIterator;
use std::{
    collections::LinkedList,
    net::{Ipv4Addr, SocketAddr},
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
//
// TODO: Maybe add IPv6 support. IPv6 may not be necessary since currently it's
// not working only in ipv6-only mode.
pub struct FakeDns {
    sender: AsyncClient,
    pool: Arc<Mutex<DnsFakeIpPool>>,
}

impl FakeDns {
    pub async fn new(
        server: SocketAddr,
        ip_range: Ipv4NetworkIterator,
        ttl: Duration,
    ) -> Result<Self> {
        let stream = UdpClientStream::<UdpSocket>::new(server);
        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        Ok(Self {
            sender: client,
            pool: Arc::new(Mutex::new(DnsFakeIpPool::new(ip_range, ttl))),
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

                let rdata = RData::A(ip);

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

    pub async fn reverse_lookup(&self, addr: &Ipv4Addr) -> Option<String> {
        self.pool.lock().await.map.get(addr).map(String::clone)
    }
}

struct DnsFakeIpPool {
    fake_ip_pool: LinkedList<Ipv4Addr>,
    ip_iter: Ipv4NetworkIterator,
    map: ExpiringHashMap<Ipv4Addr, String>,
}

impl DnsFakeIpPool {
    fn new(ip_range: Ipv4NetworkIterator, ttl: Duration) -> Self {
        Self {
            fake_ip_pool: Default::default(),
            ip_iter: ip_range,
            map: ExpiringHashMap::new(ttl.saturating_add(Duration::from_secs(5)), true),
        }
    }

    fn ttl(&self) -> u64 {
        self.map.get_ttl().as_secs().saturating_sub(5)
    }

    fn get_fake_ip(&mut self, domain: String) -> Result<Ipv4Addr> {
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
