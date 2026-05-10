//! Fans out `session_messages.parts` into `TraceEvent` records.

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::json;
use storage::messages::parts::MessagePart;
use storage::repos::Repos;

use crate::tracing::types::{HeaderStats, SemanticCategory, TraceEvent};

const PROVIDER_ID: &str = "klynt";

#[derive(Debug, Clone)]
pub struct LoadedSession {
    pub events: Vec<TraceEvent>,
    pub stats: HeaderStats,
    pub truncated: bool,
    pub total_event_count: u64,
}

pub async fn load_session(repos: &Repos, session_id: &str) -> Result<LoadedSession> {
    // Confirm session exists and pull compression markers.
    let comp_row: Option<(Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT compressed_at, compressed_through_idx FROM sessions WHERE key = ?")
            .bind(session_id)
            .fetch_optional(repos.pool())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("klynt load_session sessions: {e}")))?;
    let (compressed_at, compressed_through_idx) = match comp_row {
        Some(t) => t,
        None => {
            return Err(KlyntbotError::StorageNotFound(format!(
                "session {session_id}"
            )));
        }
    };

    let messages = repos
        .sessions
        .get_messages_parts(session_id, 5_000)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("klynt load_session messages: {e}")))?;

    let mut events: Vec<TraceEvent> = Vec::new();
    for msg in &messages {
        let ts_ms = msg.timestamp.as_millisecond();
        let occurred_at = Timestamp::from_millisecond(ts_ms).unwrap_or(Timestamp::UNIX_EPOCH);
        for (part_idx, part) in msg.parts.iter().enumerate() {
            let seq = (ts_ms as u64).saturating_mul(100) + part_idx as u64;
            if let Some(ev) =
                part_to_event(seq, occurred_at, &msg.role, msg.turn_id.as_deref(), part)
            {
                events.push(ev);
            }
        }
    }

    if let Some(comp_ms) = compressed_at {
        let occurred_at = Timestamp::from_millisecond(comp_ms).unwrap_or(Timestamp::UNIX_EPOCH);
        let seq = (comp_ms as u64).saturating_mul(100) + 99;
        events.push(TraceEvent {
            seq,
            provider_id: PROVIDER_ID.into(),
            raw_kind: "CompactionApplied".into(),
            payload: json!({
                "messages_condensed": compressed_through_idx,
            }),
            occurred_at,
            category: SemanticCategory::CompactionBegin,
            turn_index: None,
            step_index: None,
            parent_subagent_id: None,
            meta: false,
        });
    }

    events.sort_by_key(|e| e.seq);
    let total = events.len() as u64;
    let stats = compute_stats(&events);

    Ok(LoadedSession {
        events,
        stats,
        truncated: false,
        total_event_count: total,
    })
}

fn part_to_event(
    seq: u64,
    occurred_at: Timestamp,
    role: &str,
    turn_id: Option<&str>,
    part: &MessagePart,
) -> Option<TraceEvent> {
    let (raw_kind, payload, category) = match part {
        MessagePart::Text { text } if role == "user" => (
            "UserMessage",
            json!({ "text": text }),
            SemanticCategory::UserInput,
        ),
        MessagePart::Text { text } => (
            "ContentChunk",
            json!({ "text": text }),
            SemanticCategory::AssistantText,
        ),
        MessagePart::Reasoning { text, redacted } => (
            "ReasoningChunk",
            json!({ "text": text, "redacted": redacted }),
            SemanticCategory::Thinking,
        ),
        MessagePart::ToolCall {
            call_id,
            name,
            args,
        } => (
            "ToolCall",
            json!({ "call_id": call_id, "name": name, "args": args }),
            SemanticCategory::ToolCall,
        ),
        MessagePart::ToolResult {
            call_id,
            output,
            is_error,
        } => (
            "ToolResult",
            json!({
                "call_id": call_id,
                "output": output.text,
                "mime": output.mime,
                "truncated": output.truncated,
                "is_error": is_error,
            }),
            SemanticCategory::ToolResult,
        ),
        MessagePart::FileChange(data) => (
            "FileEdit",
            json!({
                "path": data.path,
                "diff_unified": data.diff_unified,
                "applied": data.applied,
            }),
            SemanticCategory::ToolResult,
        ),
        MessagePart::CommandExecution(data) => (
            "CommandExecution",
            json!({
                "command": data.command,
                "exit_code": data.exit_code,
            }),
            SemanticCategory::ToolResult,
        ),
        MessagePart::Finish { reason } => (
            "Finish",
            serde_json::to_value(reason).unwrap_or(serde_json::Value::Null),
            SemanticCategory::TurnEnd,
        ),
        MessagePart::ReviewResult {
            review_id,
            summary,
            issues,
        } => (
            "ReviewResult",
            json!({
                "review_id": review_id,
                "summary": summary,
                "issues": issues,
            }),
            SemanticCategory::Other,
        ),
    };

    Some(TraceEvent {
        seq,
        provider_id: PROVIDER_ID.into(),
        raw_kind: raw_kind.into(),
        payload,
        occurred_at,
        category,
        turn_index: turn_id.and_then(parse_turn_index),
        step_index: None,
        parent_subagent_id: None,
        meta: false,
    })
}

fn parse_turn_index(turn_id: &str) -> Option<u32> {
    // turn_id is opaque; we hash it down to a small u32 for grouping in the UI.
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    turn_id.hash(&mut h);
    Some((h.finish() % 1_000_000) as u32)
}

fn compute_stats(events: &[TraceEvent]) -> HeaderStats {
    let mut s = HeaderStats::default();
    let mut seen_turns = std::collections::HashSet::new();
    for e in events {
        match e.category {
            SemanticCategory::ToolCall => s.tool_call_count += 1,
            SemanticCategory::CompactionBegin => s.compaction_count += 1,
            SemanticCategory::Error => s.error_count += 1,
            SemanticCategory::ToolResult
                if e.payload
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false) =>
            {
                s.error_count += 1;
            }
            _ => {}
        }
        if let Some(t) = e.turn_index {
            if seen_turns.insert(t) {
                s.turn_count += 1;
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::SessionMode;
    use storage::messages::parts::{FileChangeData, MessagePart, ToolOutput};
    use storage::StoragePool;

    async fn fresh_repos() -> Repos {
        let pool = StoragePool::connect_in_memory().await.expect("memory pool");
        Repos::from_pool(&pool)
    }

    async fn empty_session(repos: &Repos, key: &str) {
        repos
            .sessions
            .upsert_session_with_mode(key, SessionMode::Coding, &serde_json::json!({}))
            .await
            .unwrap();
    }

    async fn add_msg(
        repos: &Repos,
        session_key: &str,
        role: &str,
        parts: Vec<MessagePart>,
        turn_id: &str,
    ) {
        repos
            .sessions
            .add_message_with_parts(
                session_key,
                uuid::Uuid::new_v4(),
                role,
                &parts,
                Some(turn_id),
                None,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn missing_session_returns_not_found() {
        let repos = fresh_repos().await;
        let err = load_session(&repos, "missing").await.unwrap_err();
        assert!(matches!(err, KlyntbotError::StorageNotFound(_)));
    }

    #[tokio::test]
    async fn empty_session_returns_no_events() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events.len(), 0);
        assert_eq!(out.total_event_count, 0);
    }

    #[tokio::test]
    async fn user_text_emits_user_message() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "user",
            vec![MessagePart::Text {
                text: "hello".into(),
            }],
            "t1",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].raw_kind, "UserMessage");
        assert_eq!(out.events[0].category, SemanticCategory::UserInput);
        assert_eq!(out.events[0].payload["text"], "hello");
    }

    #[tokio::test]
    async fn assistant_text_emits_content_chunk() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::Text { text: "ok".into() }],
            "t1",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events[0].raw_kind, "ContentChunk");
        assert_eq!(out.events[0].category, SemanticCategory::AssistantText);
    }

    #[tokio::test]
    async fn reasoning_emits_reasoning_chunk() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::Reasoning {
                text: "thinking".into(),
                redacted: false,
            }],
            "t1",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events[0].raw_kind, "ReasoningChunk");
        assert_eq!(out.events[0].category, SemanticCategory::Thinking);
    }

    #[tokio::test]
    async fn tool_call_and_result_pair() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::ToolCall {
                call_id: "c1".into(),
                name: "read_file".into(),
                args: serde_json::json!({"path": "/x"}),
            }],
            "t1",
        )
        .await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::ToolResult {
                call_id: "c1".into(),
                output: ToolOutput {
                    text: "contents".into(),
                    mime: None,
                    truncated: false,
                },
                is_error: false,
            }],
            "t1",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events.len(), 2);
        assert_eq!(out.events[0].raw_kind, "ToolCall");
        assert_eq!(out.events[1].raw_kind, "ToolResult");
        assert_eq!(out.stats.tool_call_count, 1);
    }

    #[tokio::test]
    async fn tool_result_with_is_error_increments_error_count() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::ToolResult {
                call_id: "c1".into(),
                output: ToolOutput {
                    text: "boom".into(),
                    mime: None,
                    truncated: false,
                },
                is_error: true,
            }],
            "t1",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.stats.error_count, 1);
    }

    #[tokio::test]
    async fn multi_part_row_emits_event_per_part_in_order() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![
                MessagePart::Text {
                    text: "narration".into(),
                },
                MessagePart::ToolCall {
                    call_id: "c1".into(),
                    name: "bash".into(),
                    args: serde_json::json!({"cmd": "ls"}),
                },
            ],
            "t1",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events.len(), 2);
        assert_eq!(out.events[0].raw_kind, "ContentChunk");
        assert_eq!(out.events[1].raw_kind, "ToolCall");
        assert!(out.events[1].seq > out.events[0].seq);
    }

    #[tokio::test]
    async fn file_change_part_emits_file_edit() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::FileChange(Box::new(FileChangeData {
                path: std::path::PathBuf::from("src/lib.rs"),
                before: None,
                after: "new".into(),
                diff_unified: "@@ -1 +1 @@\n-old\n+new".into(),
                applied: true,
            }))],
            "t1",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].raw_kind, "FileEdit");
        assert_eq!(out.events[0].payload["path"], "src/lib.rs");
    }

    #[tokio::test]
    async fn compression_synthesizes_compaction_event() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        sqlx::query(
            "UPDATE sessions SET compressed_at = ?, compressed_through_idx = ? WHERE key = ?",
        )
        .bind(1_700_000_000_000_i64)
        .bind(7_i64)
        .bind("coding:1")
        .execute(repos.pool())
        .await
        .unwrap();
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].category, SemanticCategory::CompactionBegin);
        assert_eq!(out.events[0].payload["messages_condensed"], 7);
        assert_eq!(out.stats.compaction_count, 1);
    }

    #[tokio::test]
    async fn turn_index_is_stable_per_turn_id() {
        let repos = fresh_repos().await;
        empty_session(&repos, "coding:1").await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::Text { text: "a".into() }],
            "t1",
        )
        .await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::Text { text: "b".into() }],
            "t1",
        )
        .await;
        add_msg(
            &repos,
            "coding:1",
            "assistant",
            vec![MessagePart::Text { text: "c".into() }],
            "t2",
        )
        .await;
        let out = load_session(&repos, "coding:1").await.unwrap();
        assert_eq!(out.events[0].turn_index, out.events[1].turn_index);
        assert_ne!(out.events[1].turn_index, out.events[2].turn_index);
        assert_eq!(out.stats.turn_count, 2);
    }
}
