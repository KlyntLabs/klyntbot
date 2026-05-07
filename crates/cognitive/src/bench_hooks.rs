//! Cross-thread hit counters for benchmark instrumentation.
//!
//! The KCA bench harness needs per-QA retrieval-mechanism telemetry
//! (vector hits, FTS hits, episodic hits) to attribute lift across
//! the wave ladder. Wave 0 (Truth in Telemetry) replaces the prior
//! `thread_local!` storage which silently dropped writes from tokio
//! worker threads — `record_hits` ran inside async retrieval (worker
//! thread); `read_hit_counts` ran on the bench main thread; TLS is
//! per-thread, so the bench always read zeros.
//!
//! Now backed by `OnceLock<Mutex<Inner>>`: a single shared cell visible
//! from any thread. No-op overhead is one mutex acquire/release per
//! call (negligible vs. the surrounding LLM call). Production paths
//! never read the cell so writes have no observable effect.
//!
//! See `crates/kca-bench/src/trace.rs` for the consumer side and
//! `docs/superpowers/plans/2026-05-02-kca-memory-game-changer.md` Wave 0.

use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, Default)]
pub struct HitCounts {
    pub vector_hits: u32,
    pub fts_hits: u32,
    pub episodic_hits: u32,
}

#[derive(Debug, Default)]
struct Inner {
    hits: HitCounts,
    entities: Vec<String>,
}

static GLOBAL: OnceLock<Mutex<Inner>> = OnceLock::new();

fn slot() -> &'static Mutex<Inner> {
    GLOBAL.get_or_init(|| Mutex::new(Inner::default()))
}

/// Install the bench hook (idempotent). Optional: `slot()` lazily
/// creates the cell on first access either way.
pub fn install_for_bench() {
    let _ = slot();
}

/// Reset before each QA so stale values don't leak across QAs.
pub fn reset_hit_counts() {
    let mut g = slot().lock().unwrap();
    g.hits = HitCounts::default();
    g.entities.clear();
}

/// Read once per QA after retrieval has run.
pub fn read_hit_counts() -> HitCounts {
    slot().lock().unwrap().hits
}

/// Read the entities the retrieval layer extracted for this QA.
pub fn read_entities() -> Vec<String> {
    slot().lock().unwrap().entities.clone()
}

/// Cognitive crate writes here as it observes vector/FTS/episodic hits.
/// Saturating add so a Phase 4 retry that fires retrieval twice still
/// shows total hits seen.
pub fn record_hits(vector: u32, fts: u32, episodic: u32) {
    let mut g = slot().lock().unwrap();
    g.hits.vector_hits = g.hits.vector_hits.saturating_add(vector);
    g.hits.fts_hits = g.hits.fts_hits.saturating_add(fts);
    g.hits.episodic_hits = g.hits.episodic_hits.saturating_add(episodic);
}

/// Cognitive crate writes here when entity-aware retrieval (P2) extracts
/// proper-noun candidates from the query. Empty when P2 is off or the
/// query is bare lowercase.
pub fn record_entities(entities: &[String]) {
    let mut g = slot().lock().unwrap();
    for ent in entities {
        if !g.entities.contains(ent) {
            g.entities.push(ent.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_and_accumulate() {
        reset_hit_counts();
        record_hits(2, 5, 1);
        record_hits(3, 0, 0);
        let h = read_hit_counts();
        assert_eq!(h.vector_hits, 5);
        assert_eq!(h.fts_hits, 5);
        assert_eq!(h.episodic_hits, 1);
        reset_hit_counts();
        assert_eq!(read_hit_counts().vector_hits, 0);
    }

    /// Wave 0: writes from a tokio worker thread MUST be visible from the
    /// main thread. Previously broken because `thread_local!` storage is
    /// per-thread; the bench always read zero.
    #[test]
    fn cross_thread_writes_visible() {
        reset_hit_counts();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::spawn(async {
                record_hits(7, 3, 2);
                record_entities(&["Alice".to_string(), "Bob".to_string()]);
            })
            .await
            .unwrap();
        });
        let h = read_hit_counts();
        assert_eq!(
            h.vector_hits, 7,
            "worker-thread write must be visible to main"
        );
        assert_eq!(h.fts_hits, 3);
        assert_eq!(h.episodic_hits, 2);
        assert_eq!(
            read_entities(),
            vec!["Alice".to_string(), "Bob".to_string()]
        );
    }
}
