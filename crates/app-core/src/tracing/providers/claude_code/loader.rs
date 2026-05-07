//! Streams a Claude Code session JSONL into `Vec<TraceEvent>` and `HeaderStats`.
//!
//! Single pass covering: line parsing + categorization, synthetic TurnBegin,
//! header-stat counts, token aggregation, and model selection.

use common::Result;
use jiff::Timestamp;
use serde_json::Value;
use std::path::Path;
use tokio::io::AsyncBufReadExt;

use super::categorize::{categorize_content_block, categorize_line, user_string_content_category};
use crate::tracing::types::{HeaderStats, SemanticCategory, TraceEvent};

const PROVIDER_ID: &str = "claudeCode";

#[derive(Debug, Clone)]
pub struct LoadedSession {
    pub events: Vec<TraceEvent>,
    pub stats: HeaderStats,
    pub truncated: bool,
    pub total_event_count: u64,
}

pub async fn load_session(jsonl_path: &Path) -> Result<LoadedSession> {
    let file = tokio::fs::File::open(jsonl_path)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("open jsonl: {e}")))?;
    let mut reader = tokio::io::BufReader::new(file).lines();

    let mut events: Vec<TraceEvent> = Vec::new();
    let mut seq: u64 = 0;
    let mut current_turn: Option<u32> = None;
    let mut last_turn_prompt_id: Option<String> = None;
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_cache_read: u64 = 0;
    let mut total_cache_creation: u64 = 0;
    let mut latest_model: Option<String> = None;

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("read line: {e}")))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        accumulate_usage(
            &v,
            &mut total_input,
            &mut total_output,
            &mut total_cache_read,
            &mut total_cache_creation,
        );
        track_model(&v, &mut latest_model);
        maybe_emit_turn_begin(
            &v,
            &mut events,
            &mut seq,
            &mut current_turn,
            &mut last_turn_prompt_id,
        );
        emit_for_line_with_turn(&v, &mut events, &mut seq, current_turn);
    }

    let mut stats = compute_header_stats(&events);
    stats.total_input_tokens = total_input + total_cache_creation;
    stats.total_output_tokens = total_output;
    stats.cache_read_tokens = total_cache_read;
    let denom = total_input + total_cache_creation + total_cache_read;
    stats.cache_hit_pct = if denom > 0 {
        (total_cache_read as f32 / denom as f32) * 100.0
    } else {
        0.0
    };
    stats.model = latest_model;
    let total_event_count = events.len() as u64;
    Ok(LoadedSession {
        events,
        stats,
        truncated: false,
        total_event_count,
    })
}

fn accumulate_usage(
    v: &Value,
    total_input: &mut u64,
    total_output: &mut u64,
    total_cache_read: &mut u64,
    total_cache_creation: &mut u64,
) {
    if v.get("type").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let usage = match v.get("message").and_then(|m| m.get("usage")) {
        Some(u) => u,
        None => return,
    };
    let g = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
    *total_input += g("input_tokens");
    *total_output += g("output_tokens");
    *total_cache_read += g("cache_read_input_tokens");
    *total_cache_creation += g("cache_creation_input_tokens");
}

fn track_model(v: &Value, latest: &mut Option<String>) {
    if v.get("type").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let model = v
        .get("message")
        .and_then(|m| m.get("model"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if model.is_empty() || model == "<synthetic>" {
        return;
    }
    *latest = Some(model.to_string());
}

fn maybe_emit_turn_begin(
    v: &Value,
    events: &mut Vec<TraceEvent>,
    seq: &mut u64,
    current_turn: &mut Option<u32>,
    last_turn_prompt_id: &mut Option<String>,
) {
    if v.get("type").and_then(Value::as_str) != Some("user") {
        return;
    }
    let pid = match v.get("promptId").and_then(Value::as_str) {
        Some(s) => s,
        None => return,
    };
    let has_text = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .any(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        })
        .unwrap_or(false);
    if !has_text {
        return;
    }
    if last_turn_prompt_id.as_deref() == Some(pid) {
        return;
    }
    let next = current_turn.map(|n| n + 1).unwrap_or(1);
    *current_turn = Some(next);
    *last_turn_prompt_id = Some(pid.to_string());

    let occurred_at = parse_ts(v).unwrap_or_else(Timestamp::now);
    events.push(TraceEvent {
        seq: {
            let s = *seq;
            *seq += 1;
            s
        },
        provider_id: PROVIDER_ID.to_string(),
        raw_kind: "synthetic.TurnBegin".to_string(),
        payload: serde_json::json!({"promptId":pid}),
        occurred_at,
        category: SemanticCategory::TurnBegin,
        turn_index: Some(next),
        step_index: None,
        parent_subagent_id: None,
        meta: false,
    });
}

fn emit_for_line_with_turn(
    v: &Value,
    events: &mut Vec<TraceEvent>,
    seq: &mut u64,
    current_turn: Option<u32>,
) {
    let len_before = events.len();
    emit_for_line(v, events, seq);
    for e in events.iter_mut().skip(len_before) {
        if e.turn_index.is_none() {
            e.turn_index = current_turn;
        }
    }
}

fn emit_for_line(v: &Value, events: &mut Vec<TraceEvent>, seq: &mut u64) {
    let top_kind = v.get("type").and_then(Value::as_str).unwrap_or("");
    let occurred_at = parse_ts(v).unwrap_or_else(Timestamp::now);
    let meta = v.get("isMeta").and_then(Value::as_bool).unwrap_or(false);

    match top_kind {
        "assistant" | "user" => {
            if let Some(msg) = v.get("message") {
                emit_for_message(top_kind, msg, v, occurred_at, meta, events, seq);
            }
        }
        _ => {
            let category = categorize_line(top_kind, v);
            events.push(TraceEvent {
                seq: {
                    let s = *seq;
                    *seq += 1;
                    s
                },
                provider_id: PROVIDER_ID.to_string(),
                raw_kind: top_kind.to_string(),
                payload: v.clone(),
                occurred_at,
                category,
                turn_index: None,
                step_index: None,
                parent_subagent_id: None,
                meta,
            });
        }
    }
}

fn emit_for_message(
    role: &str,
    msg: &Value,
    line: &Value,
    occurred_at: Timestamp,
    meta: bool,
    events: &mut Vec<TraceEvent>,
    seq: &mut u64,
) {
    match msg.get("content") {
        Some(Value::Array(blocks)) => {
            for block in blocks {
                let category = categorize_content_block(role, block);
                let raw_kind = format!(
                    "{}.{}",
                    role,
                    block.get("type").and_then(Value::as_str).unwrap_or("?")
                );
                events.push(TraceEvent {
                    seq: {
                        let s = *seq;
                        *seq += 1;
                        s
                    },
                    provider_id: PROVIDER_ID.to_string(),
                    raw_kind,
                    payload: block.clone(),
                    occurred_at,
                    category,
                    turn_index: None,
                    step_index: None,
                    parent_subagent_id: None,
                    meta,
                });
                let _ = line;
            }
        }
        Some(Value::String(s)) => {
            events.push(TraceEvent {
                seq: {
                    let n = *seq;
                    *seq += 1;
                    n
                },
                provider_id: PROVIDER_ID.to_string(),
                raw_kind: format!("{role}.text"),
                payload: serde_json::json!({"type":"text","text":s}),
                occurred_at,
                category: user_string_content_category(),
                turn_index: None,
                step_index: None,
                parent_subagent_id: None,
                meta,
            });
        }
        _ => {}
    }
}

fn parse_ts(v: &Value) -> Option<Timestamp> {
    v.get("timestamp")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<Timestamp>().ok())
}

fn compute_header_stats(events: &[TraceEvent]) -> HeaderStats {
    let mut s = HeaderStats::default();
    let mut first_ts: Option<Timestamp> = None;
    let mut last_ts: Option<Timestamp> = None;
    for e in events {
        match e.category {
            SemanticCategory::TurnBegin => s.turn_count += 1,
            SemanticCategory::ToolCall => s.tool_call_count += 1,
            SemanticCategory::Error => s.error_count += 1,
            SemanticCategory::CompactionBegin => s.compaction_count += 1,
            _ => {}
        }
        if first_ts.is_none() {
            first_ts = Some(e.occurred_at);
        }
        last_ts = Some(e.occurred_at);
    }
    if let (Some(a), Some(b)) = (first_ts, last_ts) {
        let dur_ms = b.duration_since(a).as_millis().max(0) as u64;
        s.total_duration_ms = dur_ms;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    async fn write_fixture(lines: &[&str]) -> tempfile::NamedTempFile {
        let f = tempfile::Builder::new()
            .suffix(".jsonl")
            .tempfile()
            .unwrap();
        let mut h = tokio::fs::File::create(f.path()).await.unwrap();
        for line in lines {
            h.write_all(line.as_bytes()).await.unwrap();
            h.write_all(b"\n").await.unwrap();
        }
        h.flush().await.unwrap();
        f
    }

    #[tokio::test]
    async fn empty_file_yields_no_events() {
        let f = write_fixture(&[]).await;
        let out = load_session(f.path()).await.unwrap();
        assert!(out.events.is_empty());
    }

    #[tokio::test]
    async fn assistant_text_emits_one_event() {
        let f = write_fixture(&[r#"{"type":"assistant","timestamp":"2026-05-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"text","text":"hi"}]}}"#]).await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].category, SemanticCategory::AssistantText);
        assert_eq!(out.events[0].raw_kind, "assistant.text");
    }

    #[tokio::test]
    async fn assistant_with_two_content_blocks_emits_two_events() {
        let f = write_fixture(&[
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":""},{"type":"tool_use","name":"Bash","input":{"command":"ls"}}]}}"#
        ]).await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.events.len(), 2);
        assert_eq!(out.events[0].category, SemanticCategory::Thinking);
        assert_eq!(out.events[1].category, SemanticCategory::ToolCall);
    }

    #[tokio::test]
    async fn system_compact_boundary_emits_compaction_begin() {
        let f = write_fixture(&[
            r#"{"type":"system","subtype":"compact_boundary","timestamp":"2026-05-01T00:00:00Z"}"#,
        ])
        .await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].category, SemanticCategory::CompactionBegin);
    }

    #[tokio::test]
    async fn is_meta_propagates() {
        let f = write_fixture(&[r#"{"type":"system","subtype":"local_command","isMeta":true,"timestamp":"2026-05-01T00:00:00Z"}"#]).await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.events.len(), 1);
        assert!(out.events[0].meta);
    }

    #[tokio::test]
    async fn unparseable_line_is_skipped() {
        let f = write_fixture(&[
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"text","text":"a"}]}}"#,
            "not json at all",
            r#"{"type":"system","subtype":"local_command","timestamp":"2026-05-01T00:00:00Z"}"#,
        ])
        .await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.events.len(), 2);
    }

    #[tokio::test]
    async fn distinct_promptid_on_user_text_emits_turn_begin() {
        let f = write_fixture(&[
            r#"{"type":"user","promptId":"P1","timestamp":"2026-05-01T00:00:00Z","message":{"role":"user","content":[{"type":"text","text":"first"}]}}"#,
            r#"{"type":"user","promptId":"P1","timestamp":"2026-05-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","is_error":false,"content":"ok"}]}}"#,
            r#"{"type":"user","promptId":"P2","timestamp":"2026-05-01T00:00:02Z","message":{"role":"user","content":[{"type":"text","text":"second"}]}}"#,
        ]).await;
        let out = load_session(f.path()).await.unwrap();
        let turns: Vec<&TraceEvent> = out
            .events
            .iter()
            .filter(|e| e.category == SemanticCategory::TurnBegin)
            .collect();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].turn_index, Some(1));
        assert_eq!(turns[1].turn_index, Some(2));
        // The user-text event after the second TurnBegin carries turn_index=2:
        let last_user_text = out
            .events
            .iter()
            .rev()
            .find(|e| e.category == SemanticCategory::UserInput)
            .unwrap();
        assert_eq!(last_user_text.turn_index, Some(2));
    }

    #[tokio::test]
    async fn tool_results_alone_do_not_trigger_turn_begin() {
        let f = write_fixture(&[
            r#"{"type":"user","promptId":"P1","timestamp":"2026-05-01T00:00:00Z","message":{"role":"user","content":[{"type":"text","text":"first"}]}}"#,
            r#"{"type":"user","promptId":"P2","timestamp":"2026-05-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","is_error":false,"content":"ok"}]}}"#,
        ]).await;
        let out = load_session(f.path()).await.unwrap();
        let turns = out
            .events
            .iter()
            .filter(|e| e.category == SemanticCategory::TurnBegin)
            .count();
        assert_eq!(
            turns, 1,
            "tool-result-only user line must not start a new turn"
        );
    }

    #[tokio::test]
    async fn header_stats_counts() {
        let f = write_fixture(&[
            r#"{"type":"user","promptId":"P1","timestamp":"2026-05-01T00:00:00Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:01Z","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","timestamp":"2026-05-01T00:00:02Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","is_error":true,"content":"boom"}]}}"#,
            r#"{"type":"system","subtype":"api_error","timestamp":"2026-05-01T00:00:03Z"}"#,
            r#"{"type":"system","subtype":"compact_boundary","timestamp":"2026-05-01T00:00:10Z","compactMetadata":{"trigger":"manual","preTokens":1,"postTokens":1,"durationMs":1}}"#,
        ]).await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.stats.turn_count, 1);
        assert_eq!(out.stats.tool_call_count, 1);
        assert_eq!(out.stats.error_count, 2, "tool_result error + api_error");
        assert_eq!(out.stats.compaction_count, 1);
        assert!(out.stats.total_duration_ms >= 10_000);
    }

    #[tokio::test]
    async fn token_aggregation_and_cache_hit_pct() {
        let f = write_fixture(&[
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:00Z","message":{"role":"assistant","model":"claude-opus-4-7","content":[{"type":"text","text":"a"}],"usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":80,"cache_creation_input_tokens":10}}}"#,
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:01Z","message":{"role":"assistant","model":"claude-opus-4-7","content":[{"type":"text","text":"b"}],"usage":{"input_tokens":2,"output_tokens":3,"cache_read_input_tokens":20,"cache_creation_input_tokens":0}}}"#,
        ]).await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.stats.total_input_tokens, (10 + 10) + 2);
        assert_eq!(out.stats.total_output_tokens, 5 + 3);
        assert_eq!(out.stats.cache_read_tokens, 80 + 20);
        let denom = (10 + 10 + 2) + (80 + 20);
        let expected = ((80 + 20) as f32 / denom as f32) * 100.0;
        assert!((out.stats.cache_hit_pct - expected).abs() < 0.001);
    }

    #[tokio::test]
    async fn model_picks_latest_excluding_synthetic() {
        let f = write_fixture(&[
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:00Z","message":{"role":"assistant","model":"claude-opus-4-6","content":[{"type":"text","text":"a"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:01Z","message":{"role":"assistant","model":"<synthetic>","content":[{"type":"text","text":"b"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:02Z","message":{"role":"assistant","model":"claude-opus-4-7","content":[{"type":"text","text":"c"}]}}"#,
        ]).await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.stats.model.as_deref(), Some("claude-opus-4-7"));
    }

    #[tokio::test]
    async fn model_only_synthetic_yields_none() {
        let f = write_fixture(&[
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:00Z","message":{"role":"assistant","model":"<synthetic>","content":[{"type":"text","text":"a"}]}}"#,
        ]).await;
        let out = load_session(f.path()).await.unwrap();
        assert_eq!(out.stats.model, None);
    }
}
