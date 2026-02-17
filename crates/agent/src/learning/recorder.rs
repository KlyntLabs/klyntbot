//! OutcomeRecorder — captures tool outcomes from agent loop hooks.
//!
//! Privacy-by-omission: only tool name, success, duration, and confidence
//! score are recorded. Tool arguments and user messages are never stored.

use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;
use tracing::warn;
use uuid::Uuid;

use crate::confidence::ConfidenceAssessment;

use super::outcome_store::OutcomeStore;
use super::types::{ExecutionMode, OutcomeRecord};

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
        execution_mode: ExecutionMode,
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
            execution_mode,
            created_at: Utc::now(),
        };

        let mut store = self.store.write().await;
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

/// AC #4 — Explicit user rating feedback.
///
/// "Explicit feedback" in the learning system means enrichment accept/override signals:
/// when the user keeps an AI suggestion (`accepted=true`) or replaces it with their
/// own value (`accepted=false`, `actual_value=Some(…)`). This is distinct from general
/// conversational feedback ("that was wrong"); the learning system captures enrichment
/// field-level signals only. Both acceptance and overrides feed the `LearningAnalyzer`
/// via `OutcomeStore::get_all_feedback()` to inform future enrichment confidence tuning.
///
/// There is no separate "rate this tool call" mechanism — enrichment feedback is the
/// sole explicit signal path. Additional explicit feedback channels are a post-v0.3 item.
#[async_trait::async_trait]
impl tools::EnrichmentFeedbackHandler for OutcomeRecorder {
    async fn record_feedback(
        &self,
        feedback: tools::EnrichmentFeedbackEntry,
    ) -> common::Result<()> {
        let mut store = self.store.write().await;
        store.record_feedback(feedback).await
    }
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
    } else if lower.contains("validation")
        || lower.contains("invalid")
        || lower.contains("missing")
    {
        "validation"
    } else if lower.contains("network") || lower.contains("connection") {
        "network"
    } else {
        "unknown"
    }
}

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
