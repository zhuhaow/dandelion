use std::{
    collections::HashMap,
    hash::Hash,
    time::{Duration, Instant},
};

pub struct ExpiringHashMap<K: Eq + Hash, V> {
    map: HashMap<K, (V, Instant)>,
    ttl: Duration,
    reset_when_access: bool,
}

impl<K: Eq + Hash, V> ExpiringHashMap<K, V> {
    pub fn new(ttl: Duration, reset_when_access: bool) -> Self {
        Self {
            map: Default::default(),
            ttl,
            reset_when_access,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.map.get_mut(key).and_then(|v| {
            if v.1.elapsed() >= self.ttl {
                None
            } else {
                if self.reset_when_access {
                    v.1 = Instant::now();
                }
                Some(&v.0)
            }
        })
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key).and_then(|v| {
            if v.1.elapsed() >= self.ttl {
                None
            } else {
                if self.reset_when_access {
                    v.1 = Instant::now();
                }
                Some(&mut v.0)
            }
        })
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.map.insert(key, (value, Instant::now()));
    }

    pub fn clear_expired(&mut self) {
        self.map.retain(|_, v| v.1.elapsed() <= self.ttl);
    }

    pub fn evict_expired(&mut self) -> Vec<(K, V)> {
        let mut drained = self.map.drain();
        let mut evicted = Vec::new();
        let mut kept = HashMap::new();

        for (k, v) in drained.by_ref() {
            if v.1.elapsed() > self.ttl {
                evicted.push((k, v.0));
            } else {
                kept.insert(k, v);
            }
        }

        drop(drained);

        self.map.extend(kept.into_iter());

        evicted
    }

    pub fn get_ttl(&self) -> Duration {
        self.ttl
    }
}
