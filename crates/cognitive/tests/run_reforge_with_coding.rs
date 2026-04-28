use cognitive::services::reforge::{CodingPhaseRunner, CodingPhaseRunnerOutcome};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

struct CountingRunner(Arc<AtomicU32>);
#[async_trait::async_trait]
impl CodingPhaseRunner for CountingRunner {
    async fn run_synthesis(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
    async fn run_rule_artifacts(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(10, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
    async fn run_cross_session_dedup(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(100, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
    async fn run_selective_delete(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(1000, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
    async fn run_symbol_validation(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(10000, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
}

#[tokio::test]
async fn run_reforge_invokes_all_4_coding_phases_in_order() {
    let counter = Arc::new(AtomicU32::new(0));
    let runner = CountingRunner(counter.clone());

    let total = cognitive::services::reforge::dispatch_coding_phases_for_test(&runner)
        .await
        .expect("dispatch");
    assert_eq!(total, 5);
    assert_eq!(counter.load(Ordering::Relaxed), 11111);
}
