//! Cross-session aggregation for the Statistics tab (Claude Code).

use common::Result;
use jiff::tz::TimeZone;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tokio::io::AsyncBufReadExt;

use super::discovery::DiscoveredSession;
use super::subagent_loader;
use crate::tracing::types::{
    ErrorByTool, ProjectTotals, StatsBundle, SubagentTypeCount, TokenSeriesPoint, ToolUsage,
};

pub async fn aggregate(sessions: &[DiscoveredSession]) -> Result<StatsBundle> {
    let mut per_project: HashMap<String, ProjectTotals> = HashMap::new();
    let mut tool_usage: HashMap<String, (u32, u32)> = HashMap::new();
    let mut tokens_by_day: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut subagent_types: HashMap<String, u32> = HashMap::new();
    let mut total_input: u64 = 0;
    let mut total_cache_creation: u64 = 0;
    let mut total_cache_read: u64 = 0;

    for s in sessions {
        let scan = scan_one(&s.jsonl_path).await?;
        let key = scan
            .cwd
            .clone()
            .unwrap_or_else(|| s.encoded_cwd.clone().unwrap_or_default());
        let entry = per_project.entry(key.clone()).or_insert(ProjectTotals {
            project_basename: scan
                .cwd
                .as_deref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            cwd: scan
                .cwd
                .clone()
                .map(std::path::PathBuf::from)
                .unwrap_or_default(),
            session_count: 0,
            turn_count: 0,
            tool_call_count: 0,
            error_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            cache_read_tokens: 0,
        });
        entry.session_count += 1;
        entry.turn_count += scan.turn_count;
        entry.tool_call_count += scan.tool_call_count;
        entry.error_count += scan.error_count;
        entry.total_input_tokens += scan.input_tokens + scan.cache_creation_tokens;
        entry.total_output_tokens += scan.output_tokens;
        entry.cache_read_tokens += scan.cache_read_tokens;

        total_input += scan.input_tokens;
        total_cache_creation += scan.cache_creation_tokens;
        total_cache_read += scan.cache_read_tokens;

        for (name, (calls, errs)) in scan.tool_calls {
            let e = tool_usage.entry(name).or_insert((0, 0));
            e.0 += calls;
            e.1 += errs;
        }
        for (day, (i, o)) in scan.daily_tokens {
            let e = tokens_by_day.entry(day).or_insert((0, 0));
            e.0 += i;
            e.1 += o;
        }

        let subs = subagent_loader::list_subagents(&s.source_dir, &s.session_id)
            .await
            .unwrap_or_default();
        for sub in subs {
            *subagent_types.entry(sub.subagent_type).or_insert(0) += 1;
        }
    }

    let cache_hit_pct = {
        let denom = total_input + total_cache_creation + total_cache_read;
        if denom > 0 {
            (total_cache_read as f32 / denom as f32) * 100.0
        } else {
            0.0
        }
    };

    let errors_by_tool = tool_usage_errors(&tool_usage);
    Ok(StatsBundle {
        per_project: per_project.into_values().collect(),
        tool_usage: tool_usage
            .into_iter()
            .map(|(t, (c, e))| ToolUsage {
                tool: t,
                call_count: c,
                error_count: e,
            })
            .collect(),
        errors_by_tool,
        token_series: tokens_by_day
            .into_iter()
            .map(|(day, (i, o))| TokenSeriesPoint {
                day,
                input_tokens: i,
                output_tokens: o,
            })
            .collect(),
        subagent_types: subagent_types
            .into_iter()
            .map(|(t, c)| SubagentTypeCount {
                subagent_type: t,
                count: c,
            })
            .collect(),
        cache_hit_pct,
    })
}

fn tool_usage_errors(map: &HashMap<String, (u32, u32)>) -> Vec<ErrorByTool> {
    map.iter()
        .filter(|(_, (_, e))| *e > 0)
        .map(|(t, (_, e))| ErrorByTool {
            tool: t.clone(),
            error_count: *e,
        })
        .collect()
}

#[derive(Default)]
struct OneScan {
    cwd: Option<String>,
    turn_count: u32,
    tool_call_count: u32,
    error_count: u32,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
    tool_calls: HashMap<String, (u32, u32)>,
    daily_tokens: BTreeMap<String, (u64, u64)>,
}

async fn scan_one(path: &Path) -> Result<OneScan> {
    let f = tokio::fs::File::open(path)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("open: {e}")))?;
    let mut r = tokio::io::BufReader::new(f).lines();
    let mut s = OneScan::default();
    let mut last_pid: Option<String> = None;
    let mut tool_id_to_name: HashMap<String, String> = HashMap::new();

    while let Some(line) = r
        .next_line()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("readline: {e}")))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if s.cwd.is_none() {
            if let Some(c) = v.get("cwd").and_then(Value::as_str) {
                s.cwd = Some(c.to_string());
            }
        }
        let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "user" => {
                if let Some(blocks) = v
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                {
                    let has_text = blocks
                        .iter()
                        .any(|b| b.get("type").and_then(Value::as_str) == Some("text"));
                    for b in blocks {
                        if b.get("type").and_then(Value::as_str) == Some("tool_result") {
                            let is_err =
                                b.get("is_error").and_then(Value::as_bool).unwrap_or(false);
                            if let Some(uid) = b.get("tool_use_id").and_then(Value::as_str) {
                                if let Some(name) = tool_id_to_name.get(uid).cloned() {
                                    let e = s.tool_calls.entry(name).or_insert((0, 0));
                                    if is_err {
                                        e.1 += 1;
                                        s.error_count += 1;
                                    }
                                }
                            }
                        }
                    }
                    if has_text {
                        if let Some(pid) = v.get("promptId").and_then(Value::as_str) {
                            if last_pid.as_deref() != Some(pid) {
                                s.turn_count += 1;
                                last_pid = Some(pid.to_string());
                            }
                        }
                    }
                }
            }
            "assistant" => {
                if let Some(blocks) = v
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                {
                    for b in blocks {
                        if b.get("type").and_then(Value::as_str) == Some("tool_use") {
                            let name = b
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let id = b
                                .get("id")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            if !id.is_empty() && !name.is_empty() {
                                tool_id_to_name.insert(id, name.clone());
                            }
                            s.tool_call_count += 1;
                            s.tool_calls.entry(name).or_insert((0, 0)).0 += 1;
                        }
                    }
                }
                if let Some(usage) = v.get("message").and_then(|m| m.get("usage")) {
                    let g = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
                    s.input_tokens += g("input_tokens");
                    s.output_tokens += g("output_tokens");
                    s.cache_read_tokens += g("cache_read_input_tokens");
                    s.cache_creation_tokens += g("cache_creation_input_tokens");
                    if let Some(ts) = v
                        .get("timestamp")
                        .and_then(Value::as_str)
                        .and_then(|x| x.parse::<jiff::Timestamp>().ok())
                    {
                        let day = ts.to_zoned(TimeZone::UTC).strftime("%Y-%m-%d").to_string();
                        let e = s.daily_tokens.entry(day).or_insert((0, 0));
                        e.0 += g("input_tokens") + g("cache_creation_input_tokens");
                        e.1 += g("output_tokens");
                    }
                }
            }
            "system" if v.get("subtype").and_then(Value::as_str) == Some("api_error") => {
                s.error_count += 1;
            }
            _ => {}
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    async fn write(lines: &[&str]) -> tempfile::NamedTempFile {
        let f = tempfile::Builder::new()
            .suffix(".jsonl")
            .tempfile()
            .unwrap();
        let mut h = tokio::fs::File::create(f.path()).await.unwrap();
        for l in lines {
            h.write_all(l.as_bytes()).await.unwrap();
            h.write_all(b"\n").await.unwrap();
        }
        h.flush().await.unwrap();
        f
    }

    #[tokio::test]
    async fn aggregate_collects_tool_usage_and_tokens() {
        let f = write(&[
            r#"{"type":"user","timestamp":"2026-05-01T00:00:00Z","cwd":"/p","promptId":"P","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#,
            r#"{"type":"assistant","timestamp":"2026-05-01T00:00:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{}}],"usage":{"input_tokens":10,"output_tokens":2,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#,
            r#"{"type":"user","timestamp":"2026-05-01T00:00:02Z","cwd":"/p","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}"#,
        ]).await;
        let d = DiscoveredSession {
            session_id: "s".into(),
            jsonl_path: f.path().to_path_buf(),
            source_dir: f.path().parent().unwrap().to_path_buf(),
            encoded_cwd: Some("-p".into()),
            imported: false,
        };
        let bundle = aggregate(&[d]).await.unwrap();
        assert_eq!(bundle.per_project.len(), 1);
        assert_eq!(
            bundle
                .tool_usage
                .iter()
                .find(|t| t.tool == "Bash")
                .unwrap()
                .call_count,
            1
        );
        let day = &bundle.token_series[0];
        assert_eq!(day.input_tokens, 10);
        assert_eq!(day.output_tokens, 2);
    }
}
