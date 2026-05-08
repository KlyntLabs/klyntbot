//! TodoSignalSource — 8th mirror source: accumulates coding-todo signals.

use ai_core::{mirror::mirror_flush_secs, AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use jiff::Timestamp;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mirror::{MirrorRepo, TodoSnapshot};

pub struct TodoSignalSource {
    repo: MirrorRepo,
    status_changes: AtomicU32,
    cancellations: AtomicU32,
    plans_proposed: AtomicU32,
    plans_ratified: AtomicU32,
    blocked_reasons: Mutex<HashMap<String, u32>>,
}

impl TodoSignalSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            status_changes: AtomicU32::new(0),
            cancellations: AtomicU32::new(0),
            plans_proposed: AtomicU32::new(0),
            plans_ratified: AtomicU32::new(0),
            blocked_reasons: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MirrorSignalSource for TodoSignalSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_todo",
            subscribed_kinds: &["StateChanged", "Cancelled", "PlanProposed", "PlanRatified"],
            flush_interval_secs: Some(mirror_flush_secs(3600)),
        }
    }

    fn name(&self) -> &'static str {
        "coding-todo-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        match signal.event_kind {
            "StateChanged" => {
                self.status_changes.fetch_add(1, Ordering::Relaxed);
            }
            "Cancelled" => {
                self.cancellations.fetch_add(1, Ordering::Relaxed);
            }
            "PlanProposed" => {
                self.plans_proposed.fetch_add(1, Ordering::Relaxed);
            }
            "PlanRatified" => {
                self.plans_ratified.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        // Accumulate blocked reason clusters from content if present.
        if !signal.content.is_empty() && signal.content.contains("blocked") {
            let mut map = self.blocked_reasons.lock().await;
            let key = first_three_words(&signal.content);
            *map.entry(key).or_insert(0) += 1;
        }

        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let status_changes = self.status_changes.load(Ordering::Relaxed);
        let cancellations = self.cancellations.load(Ordering::Relaxed);
        let plans_proposed = self.plans_proposed.load(Ordering::Relaxed);
        let plans_ratified = self.plans_ratified.load(Ordering::Relaxed);

        if status_changes == 0 && cancellations == 0 && plans_proposed == 0 && plans_ratified == 0 {
            return Ok(());
        }

        let blocked_reason_clusters: Vec<(String, u32)> = {
            let mut map = self.blocked_reasons.lock().await;
            let clusters: Vec<_> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
            map.clear();
            clusters
        };

        let snapshot = TodoSnapshot {
            id: Uuid::new_v4(),
            captured_at: Timestamp::now(),
            window_hours: 1,
            status_changes,
            cancellations,
            plans_proposed,
            plans_ratified,
            blocked_reason_clusters,
        };

        self.repo.insert_todo_snapshot(&snapshot).await?;

        // Reset counters
        self.status_changes.store(0, Ordering::Relaxed);
        self.cancellations.store(0, Ordering::Relaxed);
        self.plans_proposed.store(0, Ordering::Relaxed);
        self.plans_ratified.store(0, Ordering::Relaxed);

        Ok(())
    }
}

fn first_three_words(s: &str) -> String {
    s.split_whitespace().take(3).collect::<Vec<_>>().join(" ")
}
