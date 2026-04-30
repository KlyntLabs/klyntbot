//! Klynt Cognitive Architecture (KCA) end-to-end test harness.
//!
//! This crate intentionally contains NO production code. It only exercises
//! the public surface of `app-core`, `agent`, `cognitive`, and `coding-memory`.

pub mod asserts;
pub mod fixtures;
pub mod replayer;

pub fn init_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,kca_e2e=debug")),
        )
        .with_test_writer()
        .try_init();
}

#[cfg(test)]
mod tests {
    use crate::fixtures::{fixtures_root, load_jsonl, ConversationFixture};

    #[test]
    fn loads_seed_fixtures_without_error() {
        let root = fixtures_root();
        for f in &[
            "longmembench_subset.jsonl",
            "klynt_coding_bench.jsonl",
            "hallucination_planted.jsonl",
        ] {
            let path = root.join(f);
            assert!(path.exists(), "missing fixture {path:?}");
            let items: Vec<ConversationFixture> = load_jsonl(&path).expect("load");
            assert!(!items.is_empty(), "no items in {f}");
        }
    }
}
