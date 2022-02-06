use crate::{resolver::Resolver, Result};
use ipnetwork::Ipv4NetworkIterator;
use lru::LruCache;
use rustc_hash::{FxHashMap, FxHasher};
use std::{
    collections::LinkedList, hash::BuildHasherDefault, net::Ipv4Addr, str::FromStr, sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use trust_dns_proto::{
    op::{Message, MessageType},
    rr::{Name, RData, Record, RecordType},
};

// Only IPv4 is supported for now.
//
// TODO: Maybe add IPv6 support. IPv6 may not be necessary since currently it's
// not working only in ipv6-only mode.
pub struct FakeDns<R: Resolver> {
    server: R,
    dns_impl: Arc<Mutex<DnsImpl>>,
    ttl: Duration,
}

impl<R: Resolver> FakeDns<R> {
    pub async fn new(
        server: R,
        ip_range: Ipv4NetworkIterator,
        pool_size: usize,
        ttl: Duration,
    ) -> Self {
        Self {
            server,
            dns_impl: Arc::new(Mutex::new(DnsImpl::new(ip_range, pool_size))),
            ttl,
        }
    }

    pub async fn handle(&self, request: Message) -> Result<Message> {
        let request_domain = request.queries().iter().find_map(|q| match q.query_type() {
            RecordType::A => Some(q.name()),
            _ => None,
        });

        // The domain should be FQDN but may come with two forms, w/ or w/o
        // the ending dot. We don't want the user to deal with that.
        let request_domain = request_domain
            .map(|d| d.to_utf8())
            .map(|d| d.strip_suffix('.').unwrap_or(&d).to_owned());

        if let Some(domain) = request_domain {
            let mut dns_impl = self.dns_impl.lock().await;
            let ip = dns_impl.resolve(domain.as_str())?;

            let mut response = request.clone();
            response.set_message_type(MessageType::Response);

            let rdata = RData::A(ip);

            response.add_answer(Record::from_rdata(
                Name::from_str(domain.as_str()).unwrap(),
                self.ttl.as_secs() as u32,
                rdata,
            ));

            return Ok(response);
        }

        Ok(self.server.lookup_raw(request).await?)
    }

    pub async fn reverse_lookup(&self, addr: &Ipv4Addr) -> Option<String> {
        let mut dns_impl = self.dns_impl.lock().await;

        dns_impl.reverse_lookup(addr).map(|s| s.to_owned())
    }
}

// Tests suggest the Safari may use DNS results already expired for ~3600s. So
// we don't use a timeout for evicting expired result, but use a LRU cache with
// size capacity.
struct DnsImpl {
    fake_ip_pool: LinkedList<Ipv4Addr>,
    ip_map: LruCache<Ipv4Addr, String, BuildHasherDefault<FxHasher>>,
    domain_map: FxHashMap<String, Ipv4Addr>,
}

impl DnsImpl {
    fn new(ip_iter: Ipv4NetworkIterator, pool_size: usize) -> Self {
        Self {
            fake_ip_pool: ip_iter.take(pool_size).collect(),
            // We will handle eviction manually
            ip_map: LruCache::unbounded_with_hasher(BuildHasherDefault::default()),
            domain_map: Default::default(),
        }
    }

    fn resolve(&mut self, domain: &str) -> Result<Ipv4Addr> {
        match self.domain_map.get(domain) {
            Some(ip) => {
                self.ip_map.get(ip);
                Ok(*ip)
            }
            None => {
                // Check if we should pop ip from lru or get a new one
                if !self.fake_ip_pool.is_empty() {
                    let ip = self.fake_ip_pool.pop_front().unwrap();
                    self.ip_map.put(ip, domain.to_owned());
                    self.domain_map.insert(domain.to_owned(), ip);
                    Ok(ip)
                } else {
                    let (ip, old_domain) = self.ip_map.pop_lru().unwrap();
                    self.domain_map.remove(&old_domain);
                    self.ip_map.put(ip, domain.to_owned());
                    self.domain_map.insert(domain.to_owned(), ip);
                    Ok(ip)
                }
            }
        }
    }

    fn reverse_lookup(&mut self, ip: &Ipv4Addr) -> Option<&str> {
        self.ip_map.get(ip).map(String::as_str)
    }
}
