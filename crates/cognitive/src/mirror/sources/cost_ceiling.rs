use ai_core::{mirror::MirrorSnapshotSpec, AiSignal, MirrorSignalSource};
use async_trait::async_trait;

pub struct CostCeilingSource;

impl CostCeilingSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CostCeilingSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MirrorSignalSource for CostCeilingSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "cost_ceiling",
            subscribed_kinds: &[],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "cost-ceiling-source"
    }

    async fn accumulate(&self, _signal: &AiSignal) -> common::Result<()> {
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        Ok(())
    }
}
