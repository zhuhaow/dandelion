use crate::Result;
use anyhow::bail;
use ipnetwork::{IpNetwork, IpNetworkIterator};
use std::{
    collections::{HashMap, LinkedList},
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
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
pub struct TunDns {
    sender: AsyncClient,
    pool: Arc<Mutex<DnsFakeIpPool>>,
}

impl TunDns {
    pub async fn new(server: SocketAddr, subnet: IpNetwork) -> Result<Self> {
        let stream = UdpClientStream::<UdpSocket>::new(server);
        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        Ok(Self {
            sender: client,
            pool: Arc::new(Mutex::new(DnsFakeIpPool::new(subnet))),
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

                response.add_answer(Record::from_rdata(domain.clone(), pool.ttl(), rdata));

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
}

struct DnsFakeIpPool {
    fake_ip_pool: LinkedList<IpAddr>,
    ip_iter: IpNetworkIterator,
    map: HashMap<IpAddr, (String, Instant)>,
    ttl: u32,
}

impl DnsFakeIpPool {
    fn new(subnet: IpNetwork) -> Self {
        let mut iter = subnet.into_iter();
        // We remove the first from the iter since that suppose to be the IP of
        // DNS server. Even it's not, it doesn't hurt since it's very unlikely
        // the DNS server will be accessed by a domain, thus by checking if the
        // packet goes to DNS server first we can identify if the target IP is
        // intended to be used as a fake IP.
        iter.next();

        Self {
            fake_ip_pool: Default::default(),
            ip_iter: iter,
            map: Default::default(),
            ttl: 10,
        }
    }

    fn ttl(&self) -> u32 {
        self.ttl
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

        self.map.insert(ip, (domain, Instant::now()));

        Ok(ip)
    }

    fn fill_pool(&mut self) {
        self.clear_outdated_map();
        if self.fake_ip_pool.is_empty() {
            self.fake_ip_pool.extend(self.ip_iter.by_ref().take(10));
        }
    }

    fn clear_outdated_map(&mut self) {
        let mut released_ips: Vec<_> = Vec::new();
        self.map.retain(|k, v| {
            // We add 5 seconds here as a buffer
            if v.1.elapsed() > Duration::from_secs(self.ttl as u64 + 5) {
                released_ips.push(*k);
                false
            } else {
                true
            }
        });

        self.fake_ip_pool.extend(released_ips);
    }
}
