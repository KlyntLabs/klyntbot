//! Per-session aggregate summary computation.

use crate::tracing::providers::kimi::loader;
use crate::tracing::types::{SessionMetadataInfo, SessionSummary};
use common::Result;
use std::path::Path;

pub async fn compute(session_dir: &Path) -> Result<SessionSummary> {
    if !session_dir.exists() {
        return Err(common::KlyntbotError::StorageNotFound(format!(
            "session dir {} not found",
            session_dir.display()
        )));
    }

    let wire_path = session_dir.join("wire.jsonl");
    let context_path = session_dir.join("context.jsonl");
    let state_path = session_dir.join("state.json");
    let subagents_dir = session_dir.join("subagents");

    let wire_size = file_len(&wire_path).await;
    let context_size = file_len(&context_path).await;
    let state_size = file_len(&state_path).await;
    let total_size = wire_size + context_size + state_size;

    let detail = loader::load_session_events(&wire_path, "kimi", None, 0).await?;
    let stats = &detail.stats;

    let subagent_count = if subagents_dir.is_dir() {
        match tokio::fs::read_dir(&subagents_dir).await {
            Ok(mut rd) => {
                let mut n = 0;
                while let Ok(Some(_)) = rd.next_entry().await {
                    n += 1;
                }
                n
            }
            Err(_) => 0,
        }
    } else {
        0
    };

    let metadata = SessionMetadataInfo {
        session_id: session_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        title: session_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        title_generated: false,
        archived: false,
        archived_at: None,
        auto_archive_exempt: false,
        wire_mtime: tokio::fs::metadata(&wire_path)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
    };

    Ok(SessionSummary {
        session_id: session_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        provider_id: "kimi".into(),
        source_dir: session_dir.to_path_buf(),
        cwd: None,
        project_basename: None,
        custom_title: None,
        started_at: jiff::Timestamp::UNIX_EPOCH,
        last_event_at: jiff::Timestamp::UNIX_EPOCH,
        size_bytes: total_size,
        turn_count: stats.turn_count,
        step_count: stats.step_count,
        tool_call_count: stats.tool_call_count,
        error_count: stats.error_count,
        subagent_count: subagent_count as u32,
        has_wire: wire_path.exists(),
        has_context: context_path.exists(),
        imported: false,

        work_dir_hash: String::new(),
        has_state: state_path.exists(),
        wire_size,
        context_size,
        state_size,
        total_size,
        metadata: Some(metadata),
    })
}

async fn file_len(p: &Path) -> u64 {
    tokio::fs::metadata(p).await.map(|m| m.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001")
    }

    #[tokio::test]
    async fn computes_summary_for_existing_fixture() {
        let summary = compute(&fixture_dir()).await.unwrap();
        assert_eq!(summary.session_id, "sess-fixture-001");
        assert_eq!(summary.provider_id, "kimi");
        assert!(summary.has_wire);
        assert!(summary.has_context);
        assert!(summary.has_state);
        assert!(summary.wire_size > 0);
        assert!(summary.total_size > 0);
        assert_eq!(summary.subagent_count, 1);
        assert!(summary.metadata.is_some());
    }
}
