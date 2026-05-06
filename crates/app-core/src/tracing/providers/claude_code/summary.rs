//! Builds `SessionSummary` rows; resolves cwd from inside the JSONL.

use common::Result;
use jiff::Timestamp;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::io::AsyncBufReadExt;

use super::discovery::DiscoveredSession;
use crate::tracing::types::SessionSummary;

const PROVIDER_ID: &str = "claudeCode";

pub async fn build_summary(d: &DiscoveredSession) -> Result<SessionSummary> {
    let scan = scan_jsonl(&d.jsonl_path).await?;
    let subagent_count = {
        let sub_dir = d.source_dir.join(&d.session_id).join("subagents");
        match tokio::fs::read_dir(&sub_dir).await {
            Ok(mut entries) => {
                let mut count = 0u32;
                while let Ok(Some(e)) = entries.next_entry().await {
                    if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.ends_with(".meta.json") {
                            count += 1;
                        }
                    }
                }
                count
            }
            Err(_) => 0,
        }
    };
    let size_bytes = tokio::fs::metadata(&d.jsonl_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let cwd = scan.cwd.clone();
    let project_basename = cwd
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());
    let custom_title = scan.ai_title.or(scan.first_user_text);

    Ok(SessionSummary {
        session_id: d.session_id.clone(),
        provider_id: PROVIDER_ID.to_string(),
        source_dir: d.source_dir.clone(),
        cwd,
        project_basename,
        custom_title,
        started_at: scan.first_ts.unwrap_or_else(Timestamp::now),
        last_event_at: scan.last_ts.unwrap_or_else(Timestamp::now),
        size_bytes,
        turn_count: scan.turn_count,
        step_count: 0, // Claude Code JSONL has no step concept
        tool_call_count: scan.tool_call_count,
        error_count: scan.error_count,
        subagent_count,
        has_wire: true,
        has_context: false,
        imported: d.imported,
        work_dir_hash: d.encoded_cwd.clone().unwrap_or_default(),
        has_state: false,
        wire_size: size_bytes,
        context_size: 0,
        state_size: 0,
        total_size: size_bytes,
        metadata: None,
    })
}

#[derive(Default)]
struct Scan {
    cwd: Option<PathBuf>,
    ai_title: Option<String>,
    first_user_text: Option<String>,
    first_ts: Option<Timestamp>,
    last_ts: Option<Timestamp>,
    turn_count: u32,
    tool_call_count: u32,
    error_count: u32,
}

async fn scan_jsonl(path: &Path) -> Result<Scan> {
    let f = tokio::fs::File::open(path)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("open jsonl: {e}")))?;
    let mut reader = tokio::io::BufReader::new(f).lines();
    let mut s = Scan::default();
    let mut last_prompt_id: Option<String> = None;
    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("read line: {e}")))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str::<Value>(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(ts) = v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|x| x.parse::<Timestamp>().ok())
        {
            if s.first_ts.is_none() {
                s.first_ts = Some(ts);
            }
            s.last_ts = Some(ts);
        }
        if s.cwd.is_none() {
            if let Some(c) = v.get("cwd").and_then(Value::as_str) {
                s.cwd = Some(PathBuf::from(c));
            }
        }
        let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "ai-title" => {
                if s.ai_title.is_none() {
                    s.ai_title = v.get("aiTitle").and_then(Value::as_str).map(str::to_owned);
                }
            }
            "user" => {
                if let Some(blocks) = v
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_array)
                {
                    let has_text = blocks
                        .iter()
                        .any(|b| b.get("type").and_then(Value::as_str) == Some("text"));
                    let has_err = blocks.iter().any(|b| {
                        b.get("type").and_then(Value::as_str) == Some("tool_result")
                            && b.get("is_error").and_then(Value::as_bool).unwrap_or(false)
                    });
                    if has_err {
                        s.error_count += 1;
                    }
                    if has_text {
                        if s.first_user_text.is_none() {
                            s.first_user_text = blocks
                                .iter()
                                .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                                .and_then(|b| b.get("text").and_then(Value::as_str))
                                .map(|x| x.chars().take(80).collect::<String>());
                        }
                        if let Some(pid) = v.get("promptId").and_then(Value::as_str) {
                            if last_prompt_id.as_deref() != Some(pid) {
                                s.turn_count += 1;
                                last_prompt_id = Some(pid.to_string());
                            }
                        } else {
                            s.turn_count += 1;
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
                    s.tool_call_count += blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                        .count() as u32;
                }
            }
            "system" => {
                if v.get("subtype").and_then(Value::as_str) == Some("api_error") {
                    s.error_count += 1;
                }
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
    async fn cwd_resolved_from_first_jsonl_line_with_cwd_field() {
        let f = write(&[
            r#"{"type":"file-history-snapshot","messageId":"x","snapshot":{"messageId":"x","trackedFileBackups":{},"timestamp":"2026-05-01T00:00:00Z"}}"#,
            r#"{"type":"user","timestamp":"2026-05-01T00:00:01Z","cwd":"/Users/foo/proj","message":{"role":"user","content":[{"type":"text","text":"hi"}]},"promptId":"P"}"#,
        ]).await;
        let d = DiscoveredSession {
            session_id: "s1".into(),
            jsonl_path: f.path().to_path_buf(),
            source_dir: f.path().parent().unwrap().to_path_buf(),
            encoded_cwd: Some("-Users-foo-proj".into()),
            imported: false,
        };
        let summary = build_summary(&d).await.unwrap();
        assert_eq!(summary.cwd.as_deref(), Some(Path::new("/Users/foo/proj")));
        assert_eq!(summary.project_basename.as_deref(), Some("proj"));
    }

    #[tokio::test]
    async fn custom_title_from_ai_title_event() {
        let f = write(&[
            r#"{"type":"ai-title","aiTitle":"Fix the bug","sessionId":"s1"}"#,
            r#"{"type":"user","timestamp":"2026-05-01T00:00:00Z","cwd":"/x","promptId":"P","message":{"role":"user","content":[{"type":"text","text":"please"}]}}"#,
        ]).await;
        let d = DiscoveredSession {
            session_id: "s1".into(),
            jsonl_path: f.path().to_path_buf(),
            source_dir: f.path().parent().unwrap().to_path_buf(),
            encoded_cwd: Some("-x".into()),
            imported: false,
        };
        let summary = build_summary(&d).await.unwrap();
        assert_eq!(summary.custom_title.as_deref(), Some("Fix the bug"));
    }

    #[tokio::test]
    async fn custom_title_falls_back_to_first_user_text() {
        let f = write(&[
            r#"{"type":"user","timestamp":"2026-05-01T00:00:00Z","cwd":"/x","promptId":"P","message":{"role":"user","content":[{"type":"text","text":"please help"}]}}"#,
        ]).await;
        let d = DiscoveredSession {
            session_id: "s1".into(),
            jsonl_path: f.path().to_path_buf(),
            source_dir: f.path().parent().unwrap().to_path_buf(),
            encoded_cwd: Some("-x".into()),
            imported: false,
        };
        let summary = build_summary(&d).await.unwrap();
        assert_eq!(summary.custom_title.as_deref(), Some("please help"));
    }
}
