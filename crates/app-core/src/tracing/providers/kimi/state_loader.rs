use crate::tracing::types::{KimiTodo, SessionState};
use common::Result;
use std::path::Path;

pub async fn load_state(path: &Path) -> Result<SessionState> {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SessionState::default()),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", path.display()))),
    };
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| common::KlyntbotError::Storage(format!("parse state.json: {e}")))?;
    let custom_title = raw.get("custom_title").and_then(|v| v.as_str()).map(|s| s.to_string());
    let plan_mode = raw.get("plan_mode").and_then(|v| v.as_bool()).unwrap_or(false);
    let archived = raw.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
    let todos = raw.get("todos").and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|t| {
            let title = t.get("title")?.as_str()?.to_string();
            let status = t.get("status")?.as_str()?.to_string();
            Some(KimiTodo { title, status })
        }).collect())
        .unwrap_or_default();
    Ok(SessionState { custom_title, plan_mode, archived, todos, raw })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/state.json");
        p
    }

    #[tokio::test]
    async fn loads_fixture_state() {
        let s = load_state(&fixture()).await.unwrap();
        assert_eq!(s.custom_title.as_deref(), Some("fixture session"));
        assert!(!s.plan_mode);
        assert!(!s.archived);
        assert_eq!(s.todos.len(), 2);
        assert_eq!(s.todos[0].title, "Done thing");
        assert_eq!(s.todos[0].status, "done");
    }

    #[tokio::test]
    async fn missing_returns_default() {
        let s = load_state(&PathBuf::from("/nope")).await.unwrap();
        assert!(s.custom_title.is_none());
        assert!(s.todos.is_empty());
    }
}
