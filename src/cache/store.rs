use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use moka::future::Cache;
use moka::Expiry;

use super::stats::CacheStats;

/// Wrapper interno que carrega o TTL junto ao valor para o trait Expiry.
#[derive(Clone, Debug)]
struct Entry {
    data: Bytes,
    ttl_secs: u32,
}

/// Implementa expiração por entrada usando o TTL gravado em cada Entry.
struct SWExpiry;

impl Expiry<Bytes, Entry> for SWExpiry {
    fn expire_after_create(
        &self,
        _key: &Bytes,
        value: &Entry,
        _created_at: Instant,
    ) -> Option<Duration> {
        if value.ttl_secs > 0 {
            Some(Duration::from_secs(value.ttl_secs as u64))
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct CacheStore {
    inner: Cache<Bytes, Entry>,
    pub stats: Arc<CacheStats>,
}

impl CacheStore {
    pub fn new(max_capacity: u64) -> Self {
        let inner = Cache::builder()
            .max_capacity(max_capacity)
            .expire_after(SWExpiry)
            .build();

        Self {
            inner,
            stats: CacheStats::new(),
        }
    }

    pub async fn set(&self, key: Bytes, value: Bytes, ttl_secs: u32) {
        self.inner.insert(key, Entry { data: value, ttl_secs }).await;
        self.stats.set();
    }

    pub async fn get(&self, key: &Bytes) -> Option<Bytes> {
        let result = self.inner.get(key).await;
        if result.is_some() {
            self.stats.hit();
        } else {
            self.stats.miss();
        }
        result.map(|e| e.data)
    }

    pub async fn del(&self, key: &Bytes) -> bool {
        let existed = self.inner.contains_key(key);
        self.inner.invalidate(key).await;
        if existed {
            self.stats.delete();
        }
        existed
    }

    pub async fn flush(&self) {
        self.inner.invalidate_all();
        self.inner.run_pending_tasks().await;
        self.stats.flush();
    }

    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }
}
