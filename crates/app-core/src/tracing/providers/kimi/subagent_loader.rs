use crate::tracing::types::SubagentSummary;
use common::Result;
use jiff::Timestamp;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn list_subagents(session_dir: &Path) -> Result<Vec<SubagentSummary>> {
    let dir = session_dir.join("subagents");
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", dir.display()))),
    };
    let mut out = Vec::new();
    while let Some(s) = entries.next_entry().await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !s.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
        let agent_dir = s.path();
        let meta_path = agent_dir.join("meta.json");
        let bytes = match tokio::fs::read(&meta_path).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let agent_id = v.get("agent_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let subagent_type = v.get("subagent_type").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let description = v.get("description").and_then(|x| x.as_str()).map(|s| s.to_string());
        let created_at = secs_to_ts(v.get("created_at").and_then(|x| x.as_f64()).unwrap_or(0.0));
        let updated_at = secs_to_ts(v.get("updated_at").and_then(|x| x.as_f64()).unwrap_or(0.0));
        let event_count = count_wire_records(&agent_dir.join("wire.jsonl")).await;
        out.push(SubagentSummary {
            agent_id, subagent_type, status, description, created_at, updated_at, event_count,
        });
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(out)
}

fn secs_to_ts(s: f64) -> Timestamp {
    let secs = s.trunc() as i64;
    let nanos = ((s.fract() * 1_000_000_000.0) as i64).clamp(0, 999_999_999) as i32;
    Timestamp::new(secs, nanos).unwrap_or_else(|_| Timestamp::now())
}

async fn count_wire_records(path: &Path) -> u32 {
    let f = match tokio::fs::File::open(path).await { Ok(f) => f, Err(_) => return 0 };
    let mut reader = BufReader::new(f).lines();
    let mut n: u32 = 0;
    while let Ok(Some(line)) = reader.next_line().await {
        let t = line.trim();
        if t.is_empty() { continue; }
        // Skip metadata header.
        if t.contains("\"type\":\"metadata\"") || t.starts_with("{\"type\":\"metadata\"") { continue; }
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn session_dir() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001");
        p
    }

    #[tokio::test]
    async fn lists_fixture_subagent() {
        let v = list_subagents(&session_dir()).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].agent_id, "sub-aaa");
        assert_eq!(v[0].subagent_type, "coder");
        assert_eq!(v[0].description.as_deref(), Some("Fixture subagent"));
        // 4 wire records (excluding the metadata header).
        assert_eq!(v[0].event_count, 4);
    }

    #[tokio::test]
    async fn no_subagents_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let v = list_subagents(tmp.path()).await.unwrap();
        assert!(v.is_empty());
    }
}
