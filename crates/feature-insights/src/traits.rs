//! Dependency inversion traits for L5 access from L4.
//!
//! These traits are defined here in `feature-insights` (L4) and implemented
//! in `app-core` (L7) or `agent` (L5) where cognitive repos are available.
//! Injected into `InsightService` as `Arc<dyn Trait>` during AppCore init.

use async_trait::async_trait;
use jiff::Timestamp;

use crate::cross_domain::{EntityDomain, EntityRef};

/// A knowledge atom with its current retention percentage.
#[derive(Debug, Clone)]
pub struct AtomWithRetention {
    pub subject: String,
    pub retention_pct: f64,
}

/// Provides cognitive memory data for insight context injection.
#[async_trait]
pub trait CognitiveAccessor: Send + Sync {
    /// Search semantic facts by query text, optionally filtered by domain.
    async fn search_facts(&self, query: &str, domain: Option<&str>, limit: usize) -> Vec<String>;
    /// Get recent episodic memories mentioning a note.
    async fn recent_memories(&self, note_id: &str, limit: usize) -> Vec<String>;
    /// Get active procedural rules for a domain.
    async fn domain_rules(&self, domain: &str) -> Vec<String>;
    /// Get user model summary for a domain (deep dive only).
    async fn user_model_summary(&self, domain: &str) -> Option<String>;
    /// Get entity graph neighborhood as text (deep dive only).
    async fn entity_neighborhood(&self, note_id: &str, depth: u8) -> Vec<String>;
    /// Get temporal fact history (deep dive only).
    async fn fact_history(&self, subject: &str) -> Vec<String>;
    /// Get accepted knowledge atoms with retention data for a note.
    async fn search_atoms(&self, note_id: &str) -> Vec<AtomWithRetention>;
}

/// Provides flashcard review data for learning progress computation.
#[async_trait]
pub trait FlashcardAccessor: Send + Sync {
    /// Get average review success rate for an insight (0.0-1.0).
    async fn review_success_rate(&self, insight_review_id: &str, days: i64) -> f64;
}

/// Provides embedding operations for insight content.
#[async_trait]
pub trait InsightEmbedder: Send + Sync {
    /// Embed insight content and store in vector DB.
    async fn embed_and_store(&self, insight_id: &str, content: &str) -> Result<(), String>;
    /// Get cosine similarity between two insight embeddings (None if either missing).
    async fn similarity(&self, id_a: &str, id_b: &str) -> Option<f64>;
}

/// No-op implementations for when cognitive features are unavailable.
pub struct NoopCognitiveAccessor;

#[async_trait]
impl CognitiveAccessor for NoopCognitiveAccessor {
    async fn search_facts(&self, _: &str, _: Option<&str>, _: usize) -> Vec<String> {
        Vec::new()
    }
    async fn recent_memories(&self, _: &str, _: usize) -> Vec<String> {
        Vec::new()
    }
    async fn domain_rules(&self, _: &str) -> Vec<String> {
        Vec::new()
    }
    async fn user_model_summary(&self, _: &str) -> Option<String> {
        None
    }
    async fn entity_neighborhood(&self, _: &str, _: u8) -> Vec<String> {
        Vec::new()
    }
    async fn fact_history(&self, _: &str) -> Vec<String> {
        Vec::new()
    }
    async fn search_atoms(&self, _: &str) -> Vec<AtomWithRetention> {
        Vec::new()
    }
}

pub struct NoopFlashcardAccessor;

#[async_trait]
impl FlashcardAccessor for NoopFlashcardAccessor {
    async fn review_success_rate(&self, _: &str, _: i64) -> f64 {
        0.0
    }
}

pub struct NoopInsightEmbedder;

#[async_trait]
impl InsightEmbedder for NoopInsightEmbedder {
    async fn embed_and_store(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    async fn similarity(&self, _: &str, _: &str) -> Option<f64> {
        None
    }
}

/// A vector hit from a target domain, ready for the cross-domain heuristic.
#[derive(Debug, Clone)]
pub struct CrossDomainVectorHit {
    pub entity: EntityRef,
    pub cosine: f64,
    pub created_at: Timestamp,
}

/// Provides cross-domain vector search for the insight heuristic.
///
/// Searches LanceDB embedding tables in domains other than the source to
/// find semantically similar entities. Implemented in `app-core` where
/// `VectorStore`, `EmbeddingEngine`, and entity repos are available.
#[async_trait]
pub trait CrossDomainSearcher: Send + Sync {
    /// Search embedding tables of other domains for entities similar to the source.
    ///
    /// The implementation should:
    /// 1. Retrieve (or generate) the source entity's embedding vector.
    /// 2. For each target domain (all domains except `source_domain`), search
    ///    the corresponding LanceDB table.
    /// 3. Look up entity metadata (title, created_at) from SQLite.
    /// 4. Return the combined results.
    async fn search_other_domains(
        &self,
        source_domain: &EntityDomain,
        source_id: &str,
        source_title: &str,
    ) -> Vec<CrossDomainVectorHit>;
}

pub struct NoopCrossDomainSearcher;

#[async_trait]
impl CrossDomainSearcher for NoopCrossDomainSearcher {
    async fn search_other_domains(
        &self,
        _source_domain: &EntityDomain,
        _source_id: &str,
        _source_title: &str,
    ) -> Vec<CrossDomainVectorHit> {
        Vec::new()
    }
}
