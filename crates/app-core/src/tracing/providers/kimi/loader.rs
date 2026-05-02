//! Stream a Kimi `wire.jsonl` into `(Vec<TraceEvent>, HeaderStats)`.

use crate::tracing::providers::kimi::categorize::kimi_kind_to_category;
use crate::tracing::types::{HeaderStats, SemanticCategory, TraceEvent};
use coding_ingest::adapters::kimi_cli::wire_file::{collect_events, parse_line, WireLine};
use common::Result;
use jiff::Timestamp;
use serde_json::Value;
use std::collections::VecDeque;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

const MAX_EVENTS_DEFAULT: usize = 50_000;

pub struct LoadedSession {
    pub events: Vec<TraceEvent>,
    pub stats: HeaderStats,
    pub truncated: bool,
    pub total_event_count: u64,
}

pub async fn load_session_events(
    wire_path: &Path,
    provider_id: &str,
    parent_subagent_id: Option<String>,
    agent_count_for_main: u32,
) -> Result<LoadedSession> {
    let file = match tokio::fs::File::open(wire_path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadedSession {
                events: vec![],
                stats: HeaderStats::default(),
                truncated: false,
                total_event_count: 0,
            });
        }
        Err(e) => {
            return Err(common::KlyntbotError::Storage(format!(
                "open {}: {e}",
                wire_path.display()
            )));
        }
    };

    let mut events: VecDeque<TraceEvent> = VecDeque::with_capacity(MAX_EVENTS_DEFAULT);
    let mut stats = HeaderStats {
        agent_count: agent_count_for_main,
        ..Default::default()
    };
    let mut total: u64 = 0;
    let mut truncated = false;

    let mut turn_index: Option<u32> = None;
    let mut step_index: Option<u32> = None;
    let mut first_ts: Option<Timestamp> = None;
    let mut last_ts: Option<Timestamp> = None;
    let mut last_status_token_usage: Option<Value> = None;

    let mut reader = BufReader::new(file).lines();
    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("read line: {e}")))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let parsed = match parse_line(&line) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "skipping unparseable wire line");
                continue;
            }
        };
        match parsed {
            WireLine::Metadata(_) => continue,
            WireLine::Record(rec) => {
                total += 1;
                let ts = unix_seconds_to_ts(rec.timestamp);
                first_ts.get_or_insert(ts);
                last_ts = Some(ts);
                let collected = collect_events(&rec.message);
                for ev in collected {
                    let raw_kind = ev.kind.clone();
                    let payload = ev.payload.clone();
                    let category = kimi_kind_to_category(&raw_kind, &payload);

                    match category {
                        SemanticCategory::TurnBegin => {
                            turn_index = Some(turn_index.map_or(0, |v| v + 1));
                            step_index = None;
                            stats.turn_count += 1;
                        }
                        SemanticCategory::StepBegin => {
                            step_index = Some(step_index.map_or(0, |v| v + 1));
                            stats.step_count += 1;
                        }
                        SemanticCategory::ToolCall => stats.tool_call_count += 1,
                        SemanticCategory::Error => stats.error_count += 1,
                        SemanticCategory::CompactionBegin => stats.compaction_count += 1,
                        SemanticCategory::StatusUpdate => {
                            if let Some(usage) = payload.get("token_usage") {
                                let other = usage
                                    .get("input_other")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0);
                                let cache_read = usage
                                    .get("input_cache_read")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0);
                                let cache_creation = usage
                                    .get("input_cache_creation")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0);
                                let output =
                                    usage.get("output").and_then(Value::as_u64).unwrap_or(0);
                                stats.total_input_tokens += other + cache_read + cache_creation;
                                stats.total_output_tokens += output;
                                stats.cache_read_tokens += cache_read;
                                last_status_token_usage = Some(usage.clone());
                            }
                        }
                        _ => {}
                    }

                    events.push_back(TraceEvent {
                        seq: events.len() as u64,
                        provider_id: provider_id.to_string(),
                        raw_kind,
                        payload,
                        occurred_at: ts,
                        category,
                        turn_index,
                        step_index,
                        parent_subagent_id: parent_subagent_id.clone(),
                    });
                    if events.len() > MAX_EVENTS_DEFAULT {
                        events.pop_front();
                        truncated = true;
                    }
                }
            }
        }
    }

    if let (Some(start), Some(end)) = (first_ts, last_ts) {
        stats.total_duration_ms = (end.as_millisecond() - start.as_millisecond()).max(0) as u64;
    }
    if let Some(usage) = last_status_token_usage {
        let other = usage
            .get("input_other")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cache_read = usage
            .get("input_cache_read")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cache_creation = usage
            .get("input_cache_creation")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let denom = (other + cache_read + cache_creation) as f32;
        stats.cache_hit_pct = if denom > 0.0 {
            (cache_read as f32) / denom * 100.0
        } else {
            0.0
        };
    }

    let events: Vec<TraceEvent> = events
        .into_iter()
        .enumerate()
        .map(|(i, mut e)| {
            e.seq = i as u64;
            e
        })
        .collect();

    Ok(LoadedSession {
        events,
        stats,
        truncated,
        total_event_count: total,
    })
}

fn unix_seconds_to_ts(s: f64) -> Timestamp {
    let secs = s.trunc() as i64;
    let nanos = ((s.fract() * 1_000_000_000.0) as i64).clamp(0, 999_999_999) as i32;
    Timestamp::new(secs, nanos).unwrap_or_else(|_| Timestamp::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_wire() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/wire.jsonl");
        p
    }

    #[tokio::test]
    async fn loads_fixture_session() {
        let res = load_session_events(&fixture_wire(), "kimi", None, 1)
            .await
            .unwrap();
        // 1 turn, 2 steps, 1 tool call (paired), 0 errors, 0 compactions, 1 agent (passed in).
        assert_eq!(res.stats.turn_count, 1);
        assert_eq!(res.stats.step_count, 2);
        assert_eq!(res.stats.tool_call_count, 1);
        assert_eq!(res.stats.error_count, 0);
        assert_eq!(res.stats.compaction_count, 0);
        assert_eq!(res.stats.agent_count, 1);
        assert_eq!(res.stats.total_input_tokens, 2490 + 9216);
        assert_eq!(res.stats.total_output_tokens, 106);
        // cache_hit_pct = 9216 / (9216 + 2490 + 0) * 100 = 78.72…
        assert!((res.stats.cache_hit_pct - 78.7).abs() < 0.5);
        assert!(!res.truncated);
        // metadata line is skipped, and other 9 events become TraceEvents.
        assert_eq!(res.events.len(), 9);
        // First event is TurnBegin with category set.
        assert_eq!(res.events[0].raw_kind, "TurnBegin");
        assert_eq!(res.events[0].category, SemanticCategory::TurnBegin);
        assert_eq!(res.events[0].seq, 0);
    }

    #[tokio::test]
    async fn missing_file_returns_empty_with_zero_stats() {
        let p = std::path::PathBuf::from("/nonexistent/wire.jsonl");
        let res = load_session_events(&p, "kimi", None, 0).await.unwrap();
        assert_eq!(res.events.len(), 0);
        assert_eq!(res.stats.turn_count, 0);
    }
}
