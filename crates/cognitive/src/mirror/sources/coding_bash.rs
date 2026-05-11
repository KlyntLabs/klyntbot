//! Phase 2.3b — `BackgroundJobSignalSource`
//!
//! Subscribes to `BashJob.Completed/Failed/Cancelled/Lost` AiSignals,
//! re-reads the row from `BashJobRepo`, and writes one `EpisodicMemory`
//! per event to `episodic_memories` with `kind="bash_job"`, `domain="coding"`.

use std::sync::Arc;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use jiff::Timestamp;
use storage::repos::{BashJobRepo, BashJobRow};
use uuid::Uuid;

use crate::repos::EpisodicMemoryRepo;
use crate::types::EpisodicMemory;

const SUBSCRIBED_KINDS: &[&str] = &[
    "BashJob.Completed",
    "BashJob.Failed",
    "BashJob.Cancelled",
    "BashJob.Lost",
];

pub struct BackgroundJobSignalSource {
    episodic_repo: EpisodicMemoryRepo,
    bash_repo: Arc<BashJobRepo>,
}

impl BackgroundJobSignalSource {
    pub fn new(episodic_repo: EpisodicMemoryRepo, bash_repo: Arc<BashJobRepo>) -> Self {
        Self {
            episodic_repo,
            bash_repo,
        }
    }
}

#[async_trait]
impl MirrorSignalSource for BackgroundJobSignalSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_bash",
            subscribed_kinds: SUBSCRIBED_KINDS,
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "coding_bash"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        let job_id = match &signal.raw_event {
            Some(bus::DomainEvent::BashJob(inner)) => inner.job_id().to_string(),
            _ => return Ok(()),
        };

        let row = match self.bash_repo.get(&job_id).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                tracing::debug!(job_id, "row missing at episodic write; skipping");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(error = ?e, job_id, "bash_repo.get failed in mirror source");
                return Ok(());
            }
        };

        let mem = build_episodic_memory(&row);
        if let Err(e) = self.episodic_repo.insert(&mem).await {
            tracing::warn!(error = ?e, job_id, "episodic insert failed");
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        Ok(())
    }
}

pub fn build_episodic_memory(row: &BashJobRow) -> EpisodicMemory {
    let importance = match row.status.as_str() {
        "Failed" => 0.7,
        "Lost" => 0.6,
        "Cancelled" => 0.5,
        "Completed" => 0.3,
        _ => 0.3,
    };
    let elapsed_ms = match row.finished_at {
        Some(end) => {
            let s = row.started_at.as_millisecond() as i128;
            let e = end.as_millisecond() as i128;
            (e - s).max(0) as u64
        }
        None => 0,
    };
    let extracted: serde_json::Value = row
        .failure_extracted
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);

    let content = serde_json::json!({
        "job_id":            row.id,
        "command":           row.command,
        "command_key":       row.command_key,
        "description":       row.description,
        "status":            row.status,
        "exit_code":         row.exit_code,
        "elapsed_ms":        elapsed_ms,
        "failure_kind":      row.failure_kind,
        "failure_extracted": extracted,
    })
    .to_string();

    let summary = render_episode_summary(row, elapsed_ms);

    let metadata = serde_json::json!({
        "agent_id":  row.agent_id,
        "thread_id": row.session_id,
    })
    .to_string();

    let now = Timestamp::now().to_string();
    let occurred_at = row
        .finished_at
        .map(|t| t.to_string())
        .unwrap_or_else(|| now.clone());

    EpisodicMemory {
        id: Uuid::new_v4().to_string(),
        domain: "coding".into(),
        content,
        summary: Some(summary),
        importance,
        occurred_at,
        recorded_at: now,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "session".into(),
        scope_id: Some(row.session_id.clone()),
        scope_repo_id: None,
        metadata: Some(metadata),
        kind: Some("bash_job".into()),
        actor_id: Some(row.agent_id.clone()),
        tier: "raw".into(),
        parent_id: None,
        child_count: 0,
        rolled_up_at: None,
    }
}

fn render_episode_summary(row: &BashJobRow, elapsed_ms: u64) -> String {
    let secs = elapsed_ms as f64 / 1000.0;
    match (row.status.as_str(), row.failure_kind.as_deref()) {
        ("Completed", _) => format!("Passed `{}` in {:.1}s", truncate(&row.command, 60), secs),
        ("Cancelled", _) => format!(
            "Cancelled `{}` after {:.1}s",
            truncate(&row.command, 60),
            secs
        ),
        ("Lost", _) => format!(
            "Lost `{}` (Klynt restarted mid-run)",
            truncate(&row.command, 60)
        ),
        ("Failed", Some(kind)) => format!(
            "{} in `{}` after {:.1}s",
            kind,
            truncate(&row.command, 60),
            secs
        ),
        _ => format!("Bash job `{}` ended", truncate(&row.command, 60)),
    }
    .chars()
    .take(160)
    .collect()
}

fn truncate(s: &str, n: usize) -> &str {
    if s.len() <= n {
        s
    } else {
        let mut end = n;
        while !s.is_char_boundary(end) && end < s.len() {
            end += 1;
        }
        &s[..end.min(s.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_row(id: &str, status: &str, kind: Option<&str>) -> BashJobRow {
        BashJobRow {
            id: id.into(),
            session_id: "s1".into(),
            agent_id: "a1".into(),
            description: "desc".into(),
            command: "cargo nextest run -p agent".into(),
            command_key: "k".into(),
            cwd: "/".into(),
            timeout_ms: 60_000,
            silent_completion: false,
            status: status.into(),
            exit_code: Some(if status == "Completed" { 0 } else { 1 }),
            failure_kind: kind.map(String::from),
            failure_detail: None,
            failure_extracted: None,
            started_at: jiff::Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
            finished_at: Some(jiff::Timestamp::from_millisecond(1_700_000_005_000).unwrap()),
            total_bytes_emitted: 0,
            bisect_count: 0,
            log_path: "/tmp/x.log".into(),
            final_path: None,
            last_polled_at: None,
            last_seen_offset: 0,
        }
    }

    #[test]
    fn importance_failed() {
        let mem = build_episodic_memory(&fake_row("a", "Failed", Some("TestFailure")));
        assert!((mem.importance - 0.7).abs() < 1e-9);
        assert_eq!(mem.kind, Some("bash_job".into()));
        assert_eq!(mem.domain, "coding");
        assert_eq!(mem.scope_type, "session");
        assert_eq!(mem.scope_id, Some("s1".into()));
        assert_eq!(mem.actor_id, Some("a1".into()));
    }

    #[test]
    fn importance_completed() {
        let mem = build_episodic_memory(&fake_row("b", "Completed", None));
        assert!((mem.importance - 0.3).abs() < 1e-9);
    }

    #[test]
    fn importance_lost() {
        let mem = build_episodic_memory(&fake_row("c", "Lost", Some("Lost")));
        assert!((mem.importance - 0.6).abs() < 1e-9);
    }

    #[test]
    fn summary_under_160_chars() {
        let row = fake_row("d", "Failed", Some("TestFailure"));
        let mem = build_episodic_memory(&row);
        assert!(mem.summary.unwrap().chars().count() <= 160);
    }

    #[test]
    fn spec_returns_4_kinds() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let pool = rt.block_on(async { storage::StoragePool::connect_in_memory().await.unwrap() });
        let bash_repo = Arc::new(BashJobRepo::new(pool.inner().clone()));
        let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());
        let src = BackgroundJobSignalSource::new(ep_repo, bash_repo);
        assert_eq!(src.spec().subscribed_kinds.len(), 4);
        assert_eq!(src.spec().flush_interval_secs, None);
    }
}
