use crate::types::{HistoryEntry, SessionStatus, TrackedSession};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::warn;

/// Default Claude Code data directory. Returns `None` if home dir cannot be determined.
pub fn default_claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Discover all sessions across all projects in the Claude directory.
pub async fn discover_sessions(claude_dir: &Path) -> Vec<TrackedSession> {
    let projects_dir = claude_dir.join("projects");
    let history = load_history(claude_dir).await;
    let now = Utc::now();

    let mut sessions = Vec::new();

    let mut entries = match fs::read_dir(&projects_dir).await {
        Ok(e) => e,
        Err(e) => {
            warn!(
                "Cannot read Claude projects dir {}: {e}",
                projects_dir.display()
            );
            return sessions;
        }
    };

    while let Ok(Some(project_entry)) = entries.next_entry().await {
        let project_dir = project_entry.path();
        if !project_dir.is_dir() {
            continue;
        }

        let dir_name = project_dir
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        // Prefer the project path from history.jsonl (lossless) over directory name decoding
        // (lossy — can't distinguish path separators from literal hyphens).
        let decoded_path = decode_project_name(dir_name);

        let mut jsonl_entries = match fs::read_dir(&project_dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(file_entry)) = jsonl_entries.next_entry().await {
            let path = file_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            let session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if session_id.is_empty() {
                continue;
            }

            let metadata = file_entry.metadata().await.ok();
            let last_modified: Option<DateTime<Utc>> = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .map(DateTime::<Utc>::from);

            // Use the history project field when available (lossless path).
            let project_path = history
                .get(&session_id)
                .map(|h| h.project.clone())
                .unwrap_or_else(|| decoded_path.clone());

            let preview = history.get(&session_id).map(|h| h.display.clone());

            let status = match &last_modified {
                Some(t) => SessionStatus::from_idle_secs((now - *t).num_seconds()),
                None => SessionStatus::Completed,
            };

            sessions.push(TrackedSession {
                session_id,
                project_path: project_path.clone(),
                project_name: extract_short_name(&project_path),
                jsonl_path: path.to_string_lossy().to_string(),
                status,
                first_message_preview: preview,
                message_count: 0,
                git_branch: None,
                last_activity: last_modified,
                created_at: last_modified.unwrap_or(now),
            });
        }
    }

    // Sort by last activity (most recent first)
    sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    sessions
}

/// Load history.jsonl to map session IDs to their display prompts.
async fn load_history(claude_dir: &Path) -> HashMap<String, HistoryEntry> {
    let history_path = claude_dir.join("history.jsonl");
    let content = match fs::read_to_string(&history_path).await {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) {
            map.insert(entry.session_id.clone(), entry);
        }
    }
    map
}

/// Decode project directory name back to a path.
/// Claude encodes `/` as `-` in the directory name, so `-Users-jayden-Projects-foo`
/// becomes `/Users/jayden/Projects/foo`. Note: this is lossy for paths containing
/// literal hyphens — prefer the `project` field from history.jsonl when available.
fn decode_project_name(encoded: &str) -> String {
    encoded.replace('-', "/")
}

/// Extract the last path component as a short project name.
fn extract_short_name(project_path: &str) -> String {
    project_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(project_path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_project_name() {
        assert_eq!(
            decode_project_name("-Users-jayden-Projects-Klynt-nanobot-klyntbot"),
            "/Users/jayden/Projects/Klynt/nanobot/klyntbot"
        );
    }

    #[test]
    fn test_decode_project_name_with_hyphens() {
        // Lossy: hyphens in path components can't be distinguished from separators.
        // The history.jsonl `project` field is used as ground truth when available.
        assert_eq!(
            decode_project_name("-Users-jayden-my-project"),
            "/Users/jayden/my/project"
        );
    }

    #[test]
    fn test_extract_short_name() {
        assert_eq!(
            extract_short_name("/Users/jayden/Projects/Klynt/nanobot/klyntbot"),
            "klyntbot"
        );
        assert_eq!(extract_short_name("simple"), "simple");
    }

    #[test]
    fn test_default_claude_dir() {
        // Should return Some on most systems
        let dir = default_claude_dir();
        if let Some(d) = dir {
            assert!(d.ends_with(".claude"));
        }
    }
}
