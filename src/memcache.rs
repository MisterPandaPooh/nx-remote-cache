//! Byte-bounded LRU of small, hot artifacts. Artifacts are immutable and content-addressed,
//! so a memory copy can never be stale; the only question is budget.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;

use std::sync::Arc;

use bytes::Bytes;

use crate::auth::Context;

#[derive(Clone)]
pub struct Entry {
    pub data: Bytes,
    pub origin: Arc<Context>,
}

pub struct MemCache {
    budget: u64,
    max_item: u64,
    inner: Mutex<Inner>,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
}

#[derive(Default)]
struct Inner {
    map: HashMap<String, (Entry, u64)>, // value, generation
    order: VecDeque<(String, u64)>,     // (key, generation) oldest first; stale pairs are skipped
    bytes: u64,
    generation: u64,
}

impl MemCache {
    /// `budget == 0` disables the cache.
    pub fn new(budget: u64, max_item: u64) -> Self {
        Self {
            budget,
            max_item,
            inner: Mutex::new(Inner::default()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub fn enabled(&self) -> bool {
        self.budget > 0
    }

    pub fn get(&self, key: &str) -> Option<Entry> {
        if !self.enabled() {
            return None;
        }
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let generation = inner.generation + 1;
        inner.generation = generation;
        let found = match inner.map.get_mut(key) {
            Some((data, gen)) => {
                *gen = generation;
                Some(data.clone())
            }
            None => None,
        };
        if found.is_some() {
            inner.order.push_back((key.to_owned(), generation));
            self.hits.fetch_add(1, Relaxed);
        } else {
            self.misses.fetch_add(1, Relaxed);
        }
        found
    }

    pub fn insert(&self, key: &str, data: Bytes, origin: Context) {
        let len = data.len() as u64;
        if !self.enabled() || len > self.max_item || len > self.budget {
            return;
        }
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if inner.map.contains_key(key) {
            return;
        }
        let generation = inner.generation + 1;
        inner.generation = generation;
        inner.map.insert(
            key.to_owned(),
            (
                Entry {
                    data,
                    origin: Arc::new(origin),
                },
                generation,
            ),
        );
        inner.order.push_back((key.to_owned(), generation));
        inner.bytes += len;
        while inner.bytes > self.budget {
            let Some((old_key, gen)) = inner.order.pop_front() else {
                break;
            };
            let live = inner.map.get(&old_key).is_some_and(|(_, g)| *g == gen);
            if live {
                if let Some((old, _)) = inner.map.remove(&old_key) {
                    inner.bytes -= old.data.len() as u64;
                }
            }
        }
        // Keep the order queue from growing without bound under heavy re-reads.
        if inner.order.len() > inner.map.len() * 4 + 64 {
            let Inner { map, order, .. } = &mut *inner;
            order.retain(|(k, g)| map.get(k).is_some_and(|(_, live)| live == g));
        }
    }

    pub fn remove(&self, key: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((old, _)) = inner.map.remove(key) {
            inner.bytes -= old.data.len() as u64;
        }
    }

    pub fn stats(&self) -> MemCacheStats {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        MemCacheStats {
            enabled: self.enabled(),
            budget_bytes: self.budget,
            max_item_bytes: self.max_item,
            items: inner.map.len() as u64,
            bytes: inner.bytes,
            hits: self.hits.load(Relaxed),
            misses: self.misses.load(Relaxed),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct MemCacheStats {
    pub enabled: bool,
    pub budget_bytes: u64,
    pub max_item_bytes: u64,
    pub items: u64,
    pub bytes: u64,
    pub hits: u64,
    pub misses: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used_within_budget() {
        let c = MemCache::new(300, 200);
        c.insert("a", Bytes::from(vec![0; 100]), Context::new());
        c.insert("b", Bytes::from(vec![0; 100]), Context::new());
        c.insert("c", Bytes::from(vec![0; 100]), Context::new());
        assert_eq!(c.stats().bytes, 300);
        assert!(c.get("a").is_some()); // a becomes most recent
        c.insert("d", Bytes::from(vec![0; 100]), Context::new()); // evicts b
        assert!(c.get("b").is_none());
        assert!(c.get("a").is_some());
        assert!(c.get("c").is_some());
        assert!(c.get("d").is_some());
        assert_eq!(c.stats().bytes, 300);
    }

    #[test]
    fn respects_item_limit_and_disable() {
        let c = MemCache::new(1000, 10);
        c.insert("big", Bytes::from(vec![0; 11]), Context::new());
        assert!(c.get("big").is_none());
        let off = MemCache::new(0, 10);
        off.insert("x", Bytes::from_static(b"1"), Context::new());
        assert!(off.get("x").is_none());
        assert!(!off.enabled());
    }

    #[test]
    fn remove_frees_bytes() {
        let c = MemCache::new(100, 100);
        c.insert("a", Bytes::from(vec![0; 50]), Context::new());
        c.remove("a");
        assert_eq!(c.stats().bytes, 0);
        assert!(c.get("a").is_none());
    }
}
