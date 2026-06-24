use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub sets: AtomicU64,
    pub deletes: AtomicU64,
    pub flushes: AtomicU64,
}

impl CacheStats {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    #[inline]
    pub fn hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn set(&self) {
        self.sets.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn delete(&self) {
        self.deletes.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn flush(&self) {
        self.flushes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64 * 100.0
        };

        StatsSnapshot {
            hits,
            misses,
            sets: self.sets.load(Ordering::Relaxed),
            deletes: self.deletes.load(Ordering::Relaxed),
            flushes: self.flushes.load(Ordering::Relaxed),
            hit_rate,
        }
    }
}

#[derive(Debug)]
pub struct StatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub deletes: u64,
    pub flushes: u64,
    pub hit_rate: f64,
}

impl StatsSnapshot {
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"hits":{},"misses":{},"sets":{},"deletes":{},"flushes":{},"hit_rate_pct":{:.2}}}"#,
            self.hits, self.misses, self.sets, self.deletes, self.flushes, self.hit_rate
        )
    }
}
