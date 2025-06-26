use ipnetwork::{Ipv4NetworkIterator, Ipv6NetworkIterator};
use lru::LruCache;
use std::{
    borrow::Borrow,
    collections::{HashMap, LinkedList},
    hash::Hash,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

// Tests suggest the Safari may use DNS results already expired for ~3600s. So
// we don't use a timeout for evicting expired result, but use a LRU cache where
// we manage eviction manually.
struct PoolWithBidirectionalMapping<KeyItem: Hash + Eq, ValueItem: Hash + Eq> {
    pool: LinkedList<ValueItem>,
    mapping: HashMap<KeyItem, ValueItem>,
    reverse_mapping: LruCache<ValueItem, KeyItem>,
}

impl<KeyItem, ValueItem> PoolWithBidirectionalMapping<KeyItem, ValueItem>
where
    KeyItem: Hash + Eq + Clone,
    ValueItem: Hash + Eq + Clone,
{
    pub fn new(pool: LinkedList<ValueItem>) -> Self {
        Self {
            pool,
            mapping: HashMap::new(),
            reverse_mapping: LruCache::unbounded(),
        }
    }

    pub fn get<Q>(&mut self, key: &Q) -> Option<ValueItem>
    where
        KeyItem: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        KeyItem: for<'a> From<&'a Q>,
    {
        match self.mapping.get(key) {
            Some(value) => {
                // refresh the lru cache entry
                self.reverse_mapping.get(value);

                Some(value.clone())
            }
            // there is no mapping yet
            None => match self.pool.pop_front() {
                // we find a new value in the pool, we can use it.
                Some(value) => {
                    self.mapping.insert(key.into(), value.clone());
                    self.reverse_mapping.put(value.clone(), key.into());

                    Some(value)
                }
                // we need to evict an entry from the lru cache
                None => {
                    match self.reverse_mapping.pop_lru() {
                        Some((value, old_key)) => {
                            // If we evicted an entry, we need to remove it from the mapping
                            self.mapping.remove(old_key.borrow());

                            self.mapping.insert(key.into(), value.clone());
                            self.reverse_mapping.put(value.clone(), key.into());

                            Some(value)
                        }
                        None => {
                            // If we have no entries in the pool and no entries in the reverse mapping,
                            // we cannot return anything.
                            None
                        }
                    }
                }
            },
        }
    }

    pub fn get_reverse(&mut self, value: &ValueItem) -> Option<KeyItem> {
        self.reverse_mapping.get(value).cloned()
    }
}

enum IpType {
    Ipv4,
    Ipv6,
}

pub struct FakeDnsResolver {
    ipv4_mapping: PoolWithBidirectionalMapping<String, Ipv4Addr>,
    ipv6_mapping: PoolWithBidirectionalMapping<String, Ipv6Addr>,
}

impl FakeDnsResolver {
    pub fn new(ipv4_pool: LinkedList<Ipv4Addr>, ipv6_pool: LinkedList<Ipv6Addr>) -> Self {
        Self {
            ipv4_mapping: PoolWithBidirectionalMapping::new(ipv4_pool),
            ipv6_mapping: PoolWithBidirectionalMapping::new(ipv6_pool),
        }
    }

    fn lookup(&mut self, name: &str, query_type: IpType) -> Option<IpAddr> {
        // If the name is an IP address, return it directly.
        // Generally, the client will not query an IP address for A/AAAA records,
        // we just provide consistent behavior for clients that may do so.
        if let Ok(ip) = name.parse() {
            return Some(ip);
        }

        match query_type {
            IpType::Ipv4 => self.ipv4_mapping.get(name).map(Into::into),
            IpType::Ipv6 => self.ipv6_mapping.get(name).map(Into::into),
        }
    }

    pub fn lookup_ipv4(&mut self, name: &str) -> Option<Ipv4Addr> {
        self.lookup(name, IpType::Ipv4).and_then(|ip| match ip {
            IpAddr::V4(ipv4) => Some(ipv4),
            _ => None,
        })
    }

    pub fn lookup_ipv6(&mut self, name: &str) -> Option<Ipv6Addr> {
        self.lookup(name, IpType::Ipv6).and_then(|ip| match ip {
            IpAddr::V6(ipv6) => Some(ipv6),
            _ => None,
        })
    }

    pub fn reverse_lookup<T: Into<IpAddr>>(&mut self, addr: T) -> Option<String> {
        match addr.into() {
            IpAddr::V4(addr) => self.ipv4_mapping.get_reverse(&addr),
            IpAddr::V6(addr) => self.ipv6_mapping.get_reverse(&addr),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use ipnetwork::Ipv4Network;

    use super::*;

    #[test]
    fn fake_dns_will_return_ip_for_ip_address() {
        let mut resolver = FakeDnsResolver::new(LinkedList::new(), LinkedList::new());

        assert_eq!(
            resolver.lookup_ipv4("10.0.0.1"),
            Some("10.0.0.1".parse().unwrap())
        );

        assert_eq!(
            resolver.lookup_ipv6("2001:db8::1"),
            Some("2001:db8::1".parse().unwrap())
        );
    }

    #[test]
    fn test_pool_with_bidirectional_mapping() {
        let network = Ipv4Network::try_from("10.0.0.1/24").unwrap();

        let mut resolver = FakeDnsResolver::new(
            LinkedList::from_iter(network.iter().map(Ipv4Addr::from)),
            LinkedList::new(),
        );

        let mut set = HashSet::new();
        let mut addrs = Vec::new();

        for i in 0..network.size() {
            let name = i.to_string();
            let result = resolver.lookup_ipv4(&name);

            assert!(result.is_some());

            let ip = result.unwrap();

            assert_eq!(resolver.reverse_lookup(&ip.into()), Some(name));

            assert!(!set.contains(&ip));

            set.insert(ip);
            addrs.push(ip);
        }

        // now test lru
        for i in 0..network.size() {
            let name = i.to_string() + ".new";
            let result = resolver.lookup_ipv4(&name);

            assert!(result.is_some());

            let ip = result.unwrap();

            // we should get the same IP addresses in the same order
            assert_eq!(ip, addrs[i as usize]);

            // reverse lookup should still work
            assert_eq!(resolver.reverse_lookup(&ip.into()), Some(name));
        }
    }
}
