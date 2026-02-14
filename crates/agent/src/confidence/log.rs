//! Decision logging — append-only JSONL for confidence tracking.

use std::path::PathBuf;

use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use super::types::DecisionLogEntry;

/// Append-only JSONL logger for confidence decisions.
pub struct DecisionLogger {
    path: PathBuf,
}

impl DecisionLogger {
    /// Create a logger writing to the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Create a logger using the default path (`~/.klyntbot/decision_log.jsonl`).
    pub fn default_path() -> Self {
        let path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klyntbot")
            .join("decision_log.jsonl");
        Self::new(path)
    }

    /// Append a log entry.
    pub async fn log(&self, entry: &DecisionLogEntry) {
        if let Err(e) = self.write_entry(entry).await {
            warn!("Failed to write decision log: {}", e);
        }
    }

    async fn write_entry(&self, entry: &DecisionLogEntry) -> std::io::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut line = serde_json::to_string(entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(line.as_bytes()).await?;

        Ok(())
    }

    /// Read the most recent entries (up to `limit`).
    pub async fn recent(&self, limit: usize) -> Vec<DecisionLogEntry> {
        let content = match fs::read_to_string(&self.path).await {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        content
            .lines()
            .rev()
            .take(limit)
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::types::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn make_entry() -> DecisionLogEntry {
        DecisionLogEntry {
            id: "test-001".to_string(),
            session_key: "cli:default".to_string(),
            iteration: 1,
            tool_names: vec!["read_file".to_string()],
            user_message_preview: "read my config".to_string(),
            assessment: ConfidenceAssessment {
                score: 0.85,
                phase: AssessmentPhase::PreTool,
                reasoning: "clear intent".to_string(),
                dimensions: ConfidenceDimensions {
                    intent_clarity: 0.9,
                    tool_fit: 0.8,
                    info_sufficiency: 0.85,
                },
                action: DecisionAction::Proceed,
                assessed_at: Utc::now(),
            },
            outcome: None,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_log_and_read() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test_log.jsonl");
        let logger = DecisionLogger::new(path);

        let entry = make_entry();
        logger.log(&entry).await;
        logger.log(&entry).await;

        let recent = logger.recent(10).await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, "test-001");
    }

    #[tokio::test]
    async fn test_recent_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.jsonl");
        let logger = DecisionLogger::new(path);

        let recent = logger.recent(5).await;
        assert!(recent.is_empty());
    }
}
