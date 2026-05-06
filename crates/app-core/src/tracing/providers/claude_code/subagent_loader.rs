//! Lists subagents from `<session>/subagents/agent-*.jsonl` + `agent-*.meta.json`.

use common::Result;
use jiff::Timestamp;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use super::loader;
use crate::tracing::types::{SessionDetail, Scope, SubagentSummary};

#[derive(Debug, Deserialize)]
struct AgentMeta {
    #[serde(rename = "agentType")]
    agent_type: String,
    #[serde(default)]
    description: String,
}

pub fn subagents_dir(source_dir: &Path, session_id: &str) -> PathBuf {
    source_dir.join(session_id).join("subagents")
}

pub async fn list_subagents(source_dir: &Path, session_id: &str) -> Result<Vec<SubagentSummary>> {
    let dir = subagents_dir(source_dir, session_id);
    let mut out = Vec::new();
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", dir.display()))),
    };
    while let Some(e) = entries
        .next_entry()
        .await
        .map_err(|er| common::KlyntbotError::Storage(format!("dir iter: {er}")))?
    {
        if !e.file_type().await.is_ok_and(|t| t.is_file()) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".meta.json") {
            continue;
        }
        let stem = name.trim_end_matches(".meta.json"); // "agent-<id>"
        let agent_id = stem.strip_prefix("agent-").unwrap_or(stem).to_string();
        let jsonl_path = dir.join(format!("{stem}.jsonl"));
        if tokio::fs::metadata(&jsonl_path).await.is_err() {
            continue; // orphan meta — skip silently
        }
        let raw = match tokio::fs::read(e.path()).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let meta: AgentMeta = match serde_json::from_slice(&raw) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let md = tokio::fs::metadata(&jsonl_path).await.ok();
        let mtime = md
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|st| Timestamp::try_from(st).ok())
            .unwrap_or_else(Timestamp::now);
        let event_count = count_lines(&jsonl_path).await.unwrap_or(0);
        out.push(SubagentSummary {
            agent_id,
            subagent_type: meta.agent_type,
            status: "completed".to_string(),
            description: Some(meta.description),
            created_at: mtime,
            updated_at: mtime,
            event_count,
        });
    }
    out.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    Ok(out)
}

async fn count_lines(path: &Path) -> Result<u32> {
    use tokio::io::AsyncBufReadExt;
    let f = tokio::fs::File::open(path)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("open: {e}")))?;
    let mut r = tokio::io::BufReader::new(f).lines();
    let mut n = 0u32;
    while let Some(_l) = r
        .next_line()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("readline: {e}")))?
    {
        n += 1;
    }
    Ok(n)
}

pub async fn load_subagent_session(
    source_dir: &Path,
    session_id: &str,
    agent_id: &str,
) -> Result<SessionDetail> {
    let dir = subagents_dir(source_dir, session_id);
    let jsonl = dir.join(format!("agent-{agent_id}.jsonl"));
    let loaded = loader::load_session(&jsonl).await?;
    Ok(SessionDetail {
        session_id: agent_id.to_string(),
        provider_id: "claudeCode".to_string(),
        scope: Scope::Subagent {
            agent_id: agent_id.to_string(),
        },
        stats: loaded.stats,
        events: loaded.events,
        truncated: loaded.truncated,
        total_event_count: loaded.total_event_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn lists_subagents_pairs_meta_and_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let session_root = tmp.path().to_path_buf();
        let sub = session_root.join("sess").join("subagents");
        fs::create_dir_all(&sub).await.unwrap();
        fs::write(sub.join("agent-AAA.jsonl"), "").await.unwrap();
        fs::write(
            sub.join("agent-AAA.meta.json"),
            r#"{"agentType":"foo","description":"bar"}"#,
        )
        .await
        .unwrap();
        let r = list_subagents(&session_root, "sess").await.unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].agent_id, "AAA");
        assert_eq!(r[0].subagent_type, "foo");
        assert_eq!(r[0].description.as_deref(), Some("bar"));
    }

    #[tokio::test]
    async fn orphan_jsonl_without_meta_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("s").join("subagents");
        fs::create_dir_all(&sub).await.unwrap();
        fs::write(sub.join("agent-X.jsonl"), "").await.unwrap();
        let r = list_subagents(tmp.path(), "s").await.unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn missing_subagents_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let r = list_subagents(tmp.path(), "nope").await.unwrap();
        assert!(r.is_empty());
    }
}
