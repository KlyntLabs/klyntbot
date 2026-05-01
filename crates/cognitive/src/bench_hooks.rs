//! Thread-local hit counters for benchmark instrumentation.
//!
//! The KCA bench harness needs per-QA retrieval-mechanism telemetry
//! (vector hits, FTS hits, episodic hits) to attribute lift across
//! the four-phase improvement ladder. Plumbing this through every
//! retrieval function's return type would thrash the API; instead we
//! write to a thread-local cell from inside the cognitive crate and
//! the bench reads it once per QA.
//!
//! Bench loop is sequential per-conversation, so `thread_local!` is
//! correct — no contention, no Arc<Mutex>.
//!
//! Production paths call `record_hits` unconditionally; the writes are
//! cheap (a `Cell::get`/`set` pair) and ignored unless a bench reader
//! is paired.
//!
//! See `crates/kca-bench/src/trace.rs` for the consumer side.

use std::cell::Cell;

#[derive(Debug, Clone, Copy, Default)]
pub struct HitCounts {
    pub vector_hits: u32,
    pub fts_hits: u32,
    pub episodic_hits: u32,
}

thread_local! {
    static LAST_HIT_COUNTS: Cell<HitCounts> = const { Cell::new(HitCounts {
        vector_hits: 0,
        fts_hits: 0,
        episodic_hits: 0,
    }) };
}

/// Reset before each QA so stale values don't leak across QAs.
pub fn reset_hit_counts() {
    LAST_HIT_COUNTS.with(|c| c.set(HitCounts::default()));
}

/// Read once per QA after retrieval has run.
pub fn read_hit_counts() -> HitCounts {
    LAST_HIT_COUNTS.with(|c| c.get())
}

/// Cognitive crate writes here as it observes vector/FTS/episodic hits.
/// Saturating add so a Phase 4 retry that fires retrieval twice still
/// shows total hits seen.
pub fn record_hits(vector: u32, fts: u32, episodic: u32) {
    LAST_HIT_COUNTS.with(|c| {
        let mut h = c.get();
        h.vector_hits = h.vector_hits.saturating_add(vector);
        h.fts_hits = h.fts_hits.saturating_add(fts);
        h.episodic_hits = h.episodic_hits.saturating_add(episodic);
        c.set(h);
    });
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
}
