//! Decision logging — SQL-backed persistence for confidence tracking.

use tracing::warn;

use super::types::DecisionLogEntry;

/// SQL-backed logger for confidence decisions.
pub struct DecisionLogger {
    repo: storage::DecisionLogRepo,
}

impl DecisionLogger {
    /// Create a logger backed by a SQL repository.
    pub fn new(repo: storage::DecisionLogRepo) -> Self {
        Self { repo }
    }

    /// Append a log entry (best-effort, never propagates errors).
    pub async fn log(&self, entry: &DecisionLogEntry) {
        let row = match entry_to_row(entry) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to serialize decision log entry: {}", e);
                return;
            }
        };
        if let Err(e) = self.repo.create(&row).await {
            warn!("Failed to write decision log: {}", e);
        }
    }

    /// Read the most recent entries (up to `limit`).
    pub async fn recent(&self, limit: usize) -> Vec<DecisionLogEntry> {
        match self.repo.list_recent(limit as i64).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| row_to_entry(r).ok())
                .collect(),
            Err(e) => {
                warn!("Failed to read decision log: {}", e);
                Vec::new()
            }
        }
    }
}

/// Convert a domain entry to a SQL row.
fn entry_to_row(entry: &DecisionLogEntry) -> Result<storage::DecisionLogRow, serde_json::Error> {
    Ok(storage::DecisionLogRow {
        id: entry.id.clone(),
        session_key: entry.session_key.clone(),
        iteration: entry.iteration as i32,
        tool_names: serde_json::to_value(&entry.tool_names)?,
        user_message_preview: entry.user_message_preview.clone(),
        assessment: serde_json::to_value(&entry.assessment)?,
        outcome: entry.outcome.clone(),
        created_at: common::time::bridge::chrono_to_jiff(entry.created_at).into(),
    })
}

/// Convert a SQL row back to a domain entry.
fn row_to_entry(row: storage::DecisionLogRow) -> Result<DecisionLogEntry, serde_json::Error> {
    Ok(DecisionLogEntry {
        id: row.id,
        session_key: row.session_key,
        iteration: row.iteration as usize,
        tool_names: serde_json::from_value(row.tool_names)?,
        user_message_preview: row.user_message_preview,
        assessment: serde_json::from_value(row.assessment)?,
        outcome: row.outcome,
        created_at: common::time::bridge::jiff_to_chrono(*row.created_at),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::types::*;
    use chrono::Utc;

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

    #[test]
    fn test_entry_row_roundtrip() {
        let entry = make_entry();
        let row = entry_to_row(&entry).unwrap();
        let back = row_to_entry(row).unwrap();
        assert_eq!(back.id, "test-001");
        assert_eq!(back.session_key, "cli:default");
        assert_eq!(back.iteration, 1);
        assert_eq!(back.tool_names, vec!["read_file".to_string()]);
        assert!((back.assessment.score - 0.85).abs() < f32::EPSILON);
    }
}
