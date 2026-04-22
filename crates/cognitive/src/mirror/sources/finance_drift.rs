//! FinanceSpendingDriftSource — accumulates budget alert signals and flushes periodically.

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec}
use async_trait::async_trait;

pub struct FinanceSpendingDriftSource {
    repo: crate::mirror::MirrorRepo,
}

impl FinanceSpendingDriftSource {
    pub fn new(repo: crate::mirror::MirrorRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl MirrorSignalSource for FinanceSpendingDriftSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
        name: "finance_drift",
        subscribed_kinds: &["BudgetAlert"],
        flush_interval_secs: Some(3600),
    }

    fn name(&self) -> &'static str {
        "finance-spending-drift-source"
    }

    async fn accumulate(&self, _signal: &AiSignal) -> common::Result<()> {
        // TODO: Implement accumulation logic
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        // TODO: Implement flush logic
        Ok(())
    }
}
