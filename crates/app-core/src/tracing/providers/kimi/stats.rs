//! Cross-session aggregations for the Statistics tab.

use crate::tracing::providers::kimi::loader::load_session_events;
use crate::tracing::providers::kimi::subagent_loader::list_subagents;
use crate::tracing::types::*;
use common::Result;
use std::collections::HashMap;
use std::path::Path;

pub async fn aggregate(sessions: &[SessionSummary]) -> Result<StatsBundle> {
    let mut per_project: HashMap<String, ProjectTotals> = HashMap::new();
    let mut tool_usage: HashMap<String, ToolUsage> = HashMap::new();
    let mut subagent_types: HashMap<String, u32> = HashMap::new();
    let mut token_series: HashMap<String, TokenSeriesPoint> = HashMap::new();

    for s in sessions {
        let key = s
            .project_basename
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string());
        let entry = per_project
            .entry(key.clone())
            .or_insert_with(|| ProjectTotals {
                project_basename: key,
                cwd: s.cwd.clone().unwrap_or_default(),
                session_count: 0,
                turn_count: 0,
                tool_call_count: 0,
                error_count: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                cache_read_tokens: 0,
            });
        entry.session_count += 1;
        entry.turn_count += s.turn_count;
        entry.tool_call_count += s.tool_call_count;
        entry.error_count += s.error_count;

        // Per-day token spend keyed by `last_event_at` day.
        let day = s.last_event_at.to_string()[..10].to_string();
        let p = token_series.entry(day.clone()).or_insert(TokenSeriesPoint {
            day,
            input_tokens: 0,
            output_tokens: 0,
        });

        // Re-load each session's wire to get tool-name + token totals.
        // (Cheap because of `SessionCache`; this is the cold path.)
        let wire = s.source_dir.join("wire.jsonl");
        let loaded = load_session_events(&wire, "kimi", None, 0).await?;
        p.input_tokens += loaded.stats.total_input_tokens;
        p.output_tokens += loaded.stats.total_output_tokens;
        entry.total_input_tokens += loaded.stats.total_input_tokens;
        entry.total_output_tokens += loaded.stats.total_output_tokens;
        entry.cache_read_tokens += loaded.stats.cache_read_tokens;

        for ev in &loaded.events {
            if ev.category == SemanticCategory::ToolCall {
                let tool = ev
                    .payload
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unknown)")
                    .to_string();
                let u = tool_usage.entry(tool.clone()).or_insert(ToolUsage {
                    tool,
                    call_count: 0,
                    error_count: 0,
                });
                u.call_count += 1;
            }
            if ev.category == SemanticCategory::Error {
                if let Some(tool) = associated_tool(&loaded.events, ev.seq) {
                    if let Some(u) = tool_usage.get_mut(&tool) {
                        u.error_count += 1;
                    }
                }
            }
        }

        // Subagent type counts.
        let subs = list_subagents(&s.source_dir).await.unwrap_or_default();
        for sub in subs {
            *subagent_types.entry(sub.subagent_type).or_insert(0) += 1;
        }
    }

    let mut errors_by_tool: Vec<ErrorByTool> = tool_usage
        .values()
        .filter(|u| u.error_count > 0)
        .map(|u| ErrorByTool {
            tool: u.tool.clone(),
            error_count: u.error_count,
        })
        .collect();
    errors_by_tool.sort_by(|a, b| b.error_count.cmp(&a.error_count));

    let mut tool_usage: Vec<ToolUsage> = tool_usage.into_values().collect();
    tool_usage.sort_by(|a, b| b.call_count.cmp(&a.call_count));

    let mut per_project: Vec<ProjectTotals> = per_project.into_values().collect();
    per_project.sort_by(|a, b| b.session_count.cmp(&a.session_count));

    let mut token_series: Vec<TokenSeriesPoint> = token_series.into_values().collect();
    token_series.sort_by(|a, b| a.day.cmp(&b.day));

    let mut subagent_types: Vec<SubagentTypeCount> = subagent_types
        .into_iter()
        .map(|(t, c)| SubagentTypeCount {
            subagent_type: t,
            count: c,
        })
        .collect();
    subagent_types.sort_by(|a, b| b.count.cmp(&a.count));

    let total_input: u64 = per_project.iter().map(|p| p.total_input_tokens).sum();
    let total_cache_read: u64 = per_project.iter().map(|p| p.cache_read_tokens).sum();
    let cache_hit_pct = if total_input > 0 {
        (total_cache_read as f64 / total_input as f64 * 100.0) as f32
    } else {
        0.0
    };

    Ok(StatsBundle {
        per_project,
        tool_usage,
        errors_by_tool,
        token_series,
        subagent_types,
        cache_hit_pct,
    })
}

fn associated_tool(events: &[TraceEvent], seq: u64) -> Option<String> {
    // Look back for the matching ToolCall by payload.id.
    let result = events.iter().find(|e| e.seq == seq)?;
    let id = result.payload.get("id").and_then(|v| v.as_str())?;
    let call = events.iter().rev().find(|e| {
        e.category == SemanticCategory::ToolCall
            && e.payload.get("id").and_then(|v| v.as_str()) == Some(id)
    })?;
    call.payload
        .get("function")?
        .get("name")?
        .as_str()
        .map(|s| s.to_string())
}
