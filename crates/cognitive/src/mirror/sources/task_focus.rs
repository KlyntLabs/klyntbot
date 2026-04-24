//! TaskFocusPatternSource — accumulates task focus signals and flushes periodically.

use ai_core::{mirror::mirror_flush_secs, AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use jiff::Timestamp;
use std::sync::atomic::{AtomicU32, Ordering};
use uuid::Uuid;

use crate::mirror::{MirrorRepo, TaskFocusSnapshot};

pub struct TaskFocusPatternSource {
    repo: MirrorRepo,
    focus_changes: AtomicU32,
    tasks_completed: AtomicU32,
}

impl TaskFocusPatternSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            focus_changes: AtomicU32::new(0),
            tasks_completed: AtomicU32::new(0),
        }
    }
}

#[async_trait]
impl MirrorSignalSource for TaskFocusPatternSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "task_focus",
            subscribed_kinds: &["FocusChanged", "Completed"],
            flush_interval_secs: Some(mirror_flush_secs(3600)),
        }
    }

    fn name(&self) -> &'static str {
        "task-focus-pattern-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        match signal.event_kind {
            "FocusChanged" => {
                self.focus_changes.fetch_add(1, Ordering::Relaxed);
            }
            "Completed" => {
                self.tasks_completed.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let focus_changes = self.focus_changes.load(Ordering::Relaxed);
        let tasks_completed = self.tasks_completed.load(Ordering::Relaxed);
        if focus_changes == 0 && tasks_completed == 0 {
            return Ok(());
        }
        let completion_rate = if focus_changes > 0 {
            tasks_completed as f64 / focus_changes as f64
        } else {
            0.0
        };

        let snapshot = TaskFocusSnapshot {
            id: Uuid::new_v4(),
            captured_at: Timestamp::now(),
            window_hours: 1,
            focus_changes,
            tasks_completed,
            completion_rate,
            longest_unfinished_secs: None,
            top_tasks: vec![],
        };

        self.repo.insert_task_focus_snapshot(&snapshot).await?;

        // Reset counters
        self.focus_changes.store(0, Ordering::Relaxed);
        self.tasks_completed.store(0, Ordering::Relaxed);

        Ok(())
    }
}
