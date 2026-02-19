//! Outcome storage for the learning system.
//!
//! Supports two backends:
//! - **Pg**: SQL-backed via `OutcomeRepo` (production)
//! - **InMemory**: vector-backed (unit/integration tests without PostgreSQL)

use chrono::{DateTime, Utc};

use common::Result;

use super::types::{EnrichmentFeedbackEntry, ExecutionMode, OutcomeRecord};

/// Convert an `OutcomeRecord` (domain) to an `OutcomeRow` (SQL row).
fn outcome_to_row(outcome: &OutcomeRecord) -> Result<storage::OutcomeRow> {
    let confidence_dimensions = outcome
        .confidence_dimensions
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;

    let execution_mode = serde_json::to_value(&outcome.execution_mode)?;

    Ok(storage::OutcomeRow {
        id: outcome.id.clone(),
        session_key: outcome.session_key.clone(),
        tool_name: outcome.tool_name.clone(),
        success: outcome.success,
        error_category: outcome.error_category.clone(),
        duration_ms: outcome.duration_ms as i64,
        confidence_score: outcome.confidence_score,
        confidence_dimensions,
        execution_mode,
        created_at: outcome.created_at,
    })
}

/// Convert an `OutcomeRow` (SQL row) back to an `OutcomeRecord` (domain).
fn row_to_outcome(row: storage::OutcomeRow) -> Result<OutcomeRecord> {
    let confidence_dimensions = row
        .confidence_dimensions
        .map(serde_json::from_value)
        .transpose()?;

    let execution_mode: ExecutionMode = serde_json::from_value(row.execution_mode)?;

    Ok(OutcomeRecord {
        id: row.id,
        session_key: row.session_key,
        tool_name: row.tool_name,
        success: row.success,
        error_category: row.error_category,
        duration_ms: row.duration_ms as u64,
        confidence_score: row.confidence_score,
        confidence_dimensions,
        execution_mode,
        created_at: row.created_at,
    })
}

/// Backend for outcome storage — Pg for production, InMemory for tests.
enum Backend {
    Pg(storage::OutcomeRepo),
    InMemory {
        outcomes: std::sync::Mutex<Vec<OutcomeRecord>>,
        feedback: std::sync::Mutex<Vec<EnrichmentFeedbackEntry>>,
    },
}

/// Outcome store with pluggable backend.
pub struct OutcomeStore {
    backend: Backend,
}

impl OutcomeStore {
    /// Create a store backed by a SQL repository (production path).
    pub fn new(repo: storage::OutcomeRepo) -> Self {
        Self {
            backend: Backend::Pg(repo),
        }
    }

    /// Create an in-memory store (for tests that don't need PostgreSQL).
    pub fn new_in_memory() -> Self {
        Self {
            backend: Backend::InMemory {
                outcomes: std::sync::Mutex::new(Vec::new()),
                feedback: std::sync::Mutex::new(Vec::new()),
            },
        }
    }

    /// Record a tool execution outcome.
    pub async fn record(&self, outcome: OutcomeRecord) -> Result<()> {
        match &self.backend {
            Backend::Pg(repo) => {
                let row = outcome_to_row(&outcome)?;
                repo.create(&row)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            }
            Backend::InMemory { outcomes, .. } => {
                outcomes.lock().unwrap().push(outcome);
            }
        }
        Ok(())
    }

    /// Record enrichment feedback.
    pub async fn record_feedback(&self, feedback: EnrichmentFeedbackEntry) -> Result<()> {
        match &self.backend {
            Backend::Pg(repo) => {
                repo.create_enrichment_feedback(
                    &feedback.task_id,
                    &feedback.field,
                    &feedback.suggested_value,
                    feedback.actual_value.as_deref(),
                    feedback.accepted,
                    feedback.confidence,
                )
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            }
            Backend::InMemory { feedback: fb, .. } => {
                fb.lock().unwrap().push(feedback);
            }
        }
        Ok(())
    }

    /// Get outcomes recorded after a given timestamp.
    pub async fn outcomes_since(&self, cutoff: DateTime<Utc>) -> Result<Vec<OutcomeRecord>> {
        match &self.backend {
            Backend::Pg(repo) => {
                let rows = repo
                    .list_by_date_range(cutoff, Utc::now())
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                rows.into_iter()
                    .map(row_to_outcome)
                    .collect::<Result<Vec<_>>>()
            }
            Backend::InMemory { outcomes, .. } => {
                let guard = outcomes.lock().unwrap();
                Ok(guard
                    .iter()
                    .filter(|o| o.created_at >= cutoff)
                    .cloned()
                    .collect())
            }
        }
    }

    /// Get all outcome records (fetches from epoch to now).
    pub async fn get_all_outcomes(&self) -> Result<Vec<OutcomeRecord>> {
        match &self.backend {
            Backend::Pg(_) => self.outcomes_since(DateTime::<Utc>::MIN_UTC).await,
            Backend::InMemory { outcomes, .. } => Ok(outcomes.lock().unwrap().clone()),
        }
    }

    /// Get all enrichment feedback entries.
    pub async fn get_all_feedback(&self) -> Result<Vec<EnrichmentFeedbackEntry>> {
        match &self.backend {
            Backend::Pg(repo) => {
                let rows = repo
                    .list_enrichment_feedback()
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                Ok(rows
                    .into_iter()
                    .map(|r| EnrichmentFeedbackEntry {
                        task_id: r.task_id,
                        field: r.field,
                        suggested_value: r.suggested_value,
                        actual_value: r.actual_value,
                        accepted: r.accepted,
                        confidence: r.confidence,
                        timestamp: r.timestamp,
                    })
                    .collect())
            }
            Backend::InMemory { feedback, .. } => Ok(feedback.lock().unwrap().clone()),
        }
    }
}
