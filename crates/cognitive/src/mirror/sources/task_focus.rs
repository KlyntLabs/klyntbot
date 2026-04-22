//! TaskFocusPatternSource — accumulates task focus signals and flushes periodically.

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec}
use async_trait::async_trait;

pub struct TaskFocusPatternSource {
    repo: crate::mirror::MirrorRepo,
}

impl TaskFocusPatternSource {
    pub fn new(repo: crate::mirror::MirrorRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl MirrorSignalSource for TaskFocusPatternSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
        name: "task_focus",
        subscribed_kinds: &["TaskFocusChanged", "TaskCompleted"],
        flush_interval_secs: Some(3600),
    }

    fn name(&self) -> &'static str {
        "task-focus-pattern-source"
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
