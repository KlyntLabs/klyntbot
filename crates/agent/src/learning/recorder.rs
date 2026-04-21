//! Outcome recording — captures tool execution outcomes and enrichment feedback.
//!
//! Merges the former `outcome_store` and `recorder` modules into a single cohesive
//! module that owns both storage and recording.
//!
//! ## Privacy
//!
//! Privacy-by-omission: `OutcomeRecord` does NOT store tool arguments or user
//! messages. Only tool name, success, duration, and confidence score are persisted.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::warn;
use uuid::Uuid;

use common::Result;

use crate::confidence::ConfidenceAssessment;

use super::types::OutcomeRecord;

// ===========================================================================
// Outcome storage
// ===========================================================================

/// Convert an `OutcomeRecord` (domain) to an `OutcomeRow` (SQL row).
fn outcome_to_row(outcome: &OutcomeRecord) -> Result<storage::OutcomeRow> {
    let confidence_dimensions = outcome
        .confidence_dimensions
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;

    Ok(storage::OutcomeRow {
        id: outcome.id.clone(),
        session_key: outcome.session_key.clone(),
        tool_name: outcome.tool_name.clone(),
        success: outcome.success,
        error_category: outcome.error_category.clone(),
        duration_ms: outcome.duration_ms as i64,
        confidence_score: outcome.confidence_score,
        confidence_dimensions,
        created_at: outcome.created_at.into(),
    })
}

/// Convert an `OutcomeRow` (SQL row) back to an `OutcomeRecord` (domain).
fn row_to_outcome(row: storage::OutcomeRow) -> Result<OutcomeRecord> {
    let confidence_dimensions = row
        .confidence_dimensions
        .map(serde_json::from_value)
        .transpose()?;

    Ok(OutcomeRecord {
        id: row.id,
        session_key: row.session_key,
        tool_name: row.tool_name,
        success: row.success,
        error_category: row.error_category,
        duration_ms: row.duration_ms as u64,
        confidence_score: row.confidence_score,
        confidence_dimensions,
        created_at: *row.created_at,
    })
}

/// Backend for outcome storage — Sqlite for production, InMemory for tests.
enum Backend {
    Sqlite(storage::OutcomeRepo),
    InMemory {
        outcomes: std::sync::Mutex<Vec<OutcomeRecord>>,
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
            backend: Backend::Sqlite(repo),
        }
    }

    /// Create an in-memory store (for tests that don't need PostgreSQL).
    pub fn new_in_memory() -> Self {
        Self {
            backend: Backend::InMemory {
                outcomes: std::sync::Mutex::new(Vec::new()),
            },
        }
    }

    /// Record a tool execution outcome.
    pub async fn record(&self, outcome: OutcomeRecord) -> Result<()> {
        match &self.backend {
            Backend::Sqlite(repo) => {
                let row = outcome_to_row(&outcome)?;
                repo.create(&row).await?;
            }
            Backend::InMemory { outcomes, .. } => {
                outcomes.lock().unwrap().push(outcome);
            }
        }
        Ok(())
    }

    /// Get outcomes recorded after a given timestamp.
    pub async fn outcomes_since(&self, cutoff: jiff::Timestamp) -> Result<Vec<OutcomeRecord>> {
        match &self.backend {
            Backend::Sqlite(repo) => {
                let rows = repo
                    .list_by_date_range(cutoff, jiff::Timestamp::now())
                    .await?;
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
            Backend::Sqlite(_) => {
                self.outcomes_since(
                    jiff::Timestamp::from_second(i64::MIN / 1000)
                        .unwrap_or(jiff::Timestamp::UNIX_EPOCH),
                )
                .await
            }
            Backend::InMemory { outcomes, .. } => Ok(outcomes.lock().unwrap().clone()),
        }
    }
}

// ===========================================================================
// Outcome recorder
// ===========================================================================

/// Records tool execution outcomes into the OutcomeStore.
pub struct OutcomeRecorder {
    store: Arc<RwLock<OutcomeStore>>,
}

impl OutcomeRecorder {
    pub fn new(store: Arc<RwLock<OutcomeStore>>) -> Self {
        Self { store }
    }

    /// Record a tool execution outcome (best-effort, never propagates errors).
    #[allow(clippy::too_many_arguments)]
    pub async fn record_tool_outcome(
        &self,
        tool_name: &str,
        success: bool,
        error_category: Option<&str>,
        duration_ms: u64,
        confidence: Option<&ConfidenceAssessment>,
        session_key: &str,
    ) {
        let record = OutcomeRecord {
            id: Uuid::new_v4().to_string(),
            session_key: hash_session_key(session_key),
            tool_name: tool_name.to_string(),
            success,
            error_category: error_category.map(|s| s.to_string()),
            duration_ms,
            confidence_score: confidence.map(|c| c.score),
            confidence_dimensions: confidence.map(|c| c.dimensions.clone()),
            created_at: jiff::Timestamp::now(),
        };

        let store = self.store.write().await;
        if let Err(e) = store.record(record).await {
            warn!("Failed to record tool outcome: {}", e);
        }
    }
}

/// Hash a session key, keeping the channel prefix for analytics.
/// Uses a simple FNV-like hash to avoid adding crypto dependencies.
/// "telegram:abc123def" → "telegram:a1b2c3d4"
fn hash_session_key(key: &str) -> String {
    if let Some((prefix, suffix)) = key.split_once(':') {
        format!("{}:{:08x}", prefix, fnv_hash(suffix.as_bytes()))
    } else {
        format!("{:08x}", fnv_hash(key.as_bytes()))
    }
}

/// Simple FNV-1a 32-bit hash — sufficient for privacy pseudonymization.
fn fnv_hash(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// Categorize an error message into a broad category.
pub fn categorize_error(error_msg: &str) -> &'static str {
    let lower = error_msg.to_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        "timeout"
    } else if lower.contains("permission")
        || lower.contains("denied")
        || lower.contains("forbidden")
    {
        "permission"
    } else if lower.contains("not found") || lower.contains("no such") {
        "not_found"
    } else if lower.contains("validation") || lower.contains("invalid") || lower.contains("missing")
    {
        "validation"
    } else if lower.contains("network") || lower.contains("connection") {
        "network"
    } else {
        "unknown"
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_session_key_preserves_prefix() {
        let hashed = hash_session_key("telegram:abc123");
        assert!(hashed.starts_with("telegram:"));
        assert_ne!(hashed, "telegram:abc123");
    }

    #[test]
    fn test_hash_session_key_deterministic() {
        let h1 = hash_session_key("telegram:abc123");
        let h2 = hash_session_key("telegram:abc123");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_session_key_no_prefix() {
        let hashed = hash_session_key("noprefix");
        assert!(!hashed.is_empty());
    }

    #[test]
    fn test_hash_session_key_different_keys_different_hashes() {
        let h1 = hash_session_key("telegram:user1");
        let h2 = hash_session_key("telegram:user2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_categorize_error() {
        assert_eq!(categorize_error("Connection timed out"), "timeout");
        assert_eq!(categorize_error("Permission denied"), "permission");
        assert_eq!(categorize_error("File not found"), "not_found");
        assert_eq!(categorize_error("Invalid parameter"), "validation");
        assert_eq!(categorize_error("Network error"), "network");
        assert_eq!(categorize_error("Something went wrong"), "unknown");
    }
}
