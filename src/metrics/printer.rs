use std::sync::Arc;
use std::time::Duration;

use tokio::time;

use crate::cache::stats::CacheStats;
use crate::cache::store::CacheStore;

/// Imprime um painel de métricas no stdout a cada `interval_secs` segundos.
/// Mostra tanto os acumulados totais quanto os deltas da última janela.
pub async fn run_metrics_printer(store: CacheStore, interval_secs: u64) {
    let stats: Arc<CacheStats> = store.stats.clone();
    let mut interval = time::interval(Duration::from_secs(interval_secs));
    interval.tick().await; // descarta o tick imediato na inicialização

    let mut prev_hits    = 0u64;
    let mut prev_misses  = 0u64;
    let mut prev_sets    = 0u64;
    let mut prev_deletes = 0u64;

    loop {
        interval.tick().await;

        let snap    = stats.snapshot();
        let entries = store.entry_count();

        let d_hits    = snap.hits.saturating_sub(prev_hits);
        let d_misses  = snap.misses.saturating_sub(prev_misses);
        let d_sets    = snap.sets.saturating_sub(prev_sets);
        let d_deletes = snap.deletes.saturating_sub(prev_deletes);
        let d_total   = d_hits + d_misses;
        let d_hit_rate = if d_total == 0 { 0.0 } else { d_hits as f64 / d_total as f64 * 100.0 };
        let ops_per_sec = (d_hits + d_misses + d_sets + d_deletes) as f64 / interval_secs as f64;

        println!(
            "\n┌─────────────────────────────── WireCache ────────────────────────────────┐\
           \n│  entries: {:>10}   hit rate: {:>5.1}%   ops/s: {:>8.0}              │\
           \n│  total   │  hits: {:>10}   misses: {:>10}   sets: {:>10}  │\
           \n│  Δ/{:>3}s  │  hits: {:>10}   misses: {:>10}   sets: {:>10}  │\
           \n│           deletes: {:>10}   flushes: {:>9}   Δ hit rate: {:>4.1}%  │\
           \n└───────────────────────────────────────────────────────────────────────────┘",
            entries,
            snap.hit_rate,
            ops_per_sec,
            snap.hits,
            snap.misses,
            snap.sets,
            interval_secs,
            d_hits,
            d_misses,
            d_sets,
            snap.deletes,
            snap.flushes,
            d_hit_rate,
        );

        prev_hits    = snap.hits;
        prev_misses  = snap.misses;
        prev_sets    = snap.sets;
        prev_deletes = snap.deletes;
    }
}
