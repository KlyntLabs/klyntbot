use jiff::Timestamp;
use common::truncate_at_boundary;
use storage::{sanitize_predicate_value, VectorStore};

use crate::embedder::TextEmbedder;

/// Configuration for conversation recall time-decay and search defaults.
#[derive(Debug, Clone)]
pub struct RecallConfig {
    pub decay_half_life_days: f64,
    pub default_threshold: f32,
    pub default_limit: usize,
}

impl Default for RecallConfig {
    fn default() -> Self {
        Self {
            decay_half_life_days: 138.0, // ~0.995/day
            default_threshold: 0.4,
            default_limit: 5,
        }
    }
}

/// A single conversation recall result with time-decayed score.
#[derive(Debug, Clone)]
pub struct RecallResult {
    pub id: String,
    pub session_key: String,
    pub role: String,
    pub content: String,
    pub score: f64,
    pub created_at: Timestamp,
}

/// Metadata for a conversation message being stored.
#[derive(Debug, Clone)]
pub struct RecallMetadata {
    pub session_key: String,
    pub role: String,
}

/// Owns all conversation recall operations: embed, search, prune.
///
/// Lives in the cognitive crate as the single owner of conversation memory.
/// Embedding is delegated to a `TextEmbedder` (implemented in `agent`).
pub struct ConversationRecallService {
    vector_store: VectorStore,
    embedder: std::sync::Arc<dyn TextEmbedder>,
    config: RecallConfig,
    /// Precomputed per-day decay factor: `0.5^(1/half_life_days)`
    decay_factor: f64,
}

impl ConversationRecallService {
    pub fn new(
        vector_store: VectorStore,
        embedder: std::sync::Arc<dyn TextEmbedder>,
        config: RecallConfig,
    ) -> Self {
        let decay_factor = 0.5_f64.powf(1.0 / config.decay_half_life_days);
        Self {
            vector_store,
            embedder,
            config,
            decay_factor,
        }
    }

    pub fn config(&self) -> &RecallConfig {
        &self.config
    }

    /// Embed and store a conversation message for future recall.
    ///
    /// Composes text as "{role}: {content}" before embedding, matching the
    /// convention from the previous `ConversationEmbeddingHandlerImpl`.
    pub async fn store_message(
        &self,
        id: &str,
        content: &str,
        metadata: RecallMetadata,
    ) -> common::Result<()> {
        let text = format!("{}: {}", metadata.role, content);
        let vector = self.embedder.embed(&text).await?;

        let preview = truncate_at_boundary(content, 100);

        self.vector_store
            .upsert_embedding(
                "conv_embeddings",
                id,
                &vector,
                &[
                    ("session_key", metadata.session_key.as_str()),
                    ("role", metadata.role.as_str()),
                    ("content_preview", preview),
                ],
            )
            .await?;
        Ok(())
    }

    /// Search past conversations with time-decay scoring.
    ///
    /// Fetches extra candidates from LanceDB so time-decay filtering still
    /// yields `limit` results when some fall below threshold.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> common::Result<Vec<RecallResult>> {
        let vector = self.embedder.embed(query).await?;

        // search_conv_embeddings returns 6-tuples:
        // (id, session_key, role, content_preview, created_at_str, score)
        let raw_results = self
            .vector_store
            .search_conv_embeddings(&vector, limit * 2, threshold as f64)
            .await?;

        let now = Timestamp::now();

        let mut results: Vec<RecallResult> = raw_results
            .into_iter()
            .filter_map(
                |(id, session_key, role, preview, created_at_str, similarity)| {
                    let created_at = created_at_str.parse::<Timestamp>().unwrap_or(Timestamp::now());

                    let days_old = (now.as_millisecond() - created_at.as_millisecond()) as f64 / 86_400_000.0;
                    let decayed_score = similarity * self.decay_factor.powf(days_old.max(0.0));

                    if decayed_score >= threshold as f64 {
                        Some(RecallResult {
                            id,
                            session_key,
                            role,
                            content: preview,
                            score: decayed_score,
                            created_at,
                        })
                    } else {
                        None
                    }
                },
            )
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// Delete conversation embeddings older than the given cutoff.
    pub async fn delete_older_than(&self, cutoff: Timestamp) -> common::Result<()> {
        let cutoff_str = sanitize_predicate_value(&cutoff.to_string())?;
        self.vector_store
            .delete_where("conv_embeddings", &format!("created_at < '{cutoff_str}'"))
            .await?;
        Ok(())
    }

    /// Delete all conversation embeddings for a specific session.
    pub async fn delete_by_session_key(&self, session_key: &str) -> common::Result<()> {
        let escaped = sanitize_predicate_value(session_key)?;
        self.vector_store
            .delete_where("conv_embeddings", &format!("session_key = '{escaped}'"))
            .await?;
        Ok(())
    }

    /// Delete all conversation embeddings.
    pub async fn delete_all(&self) -> common::Result<()> {
        self.vector_store
            .delete_where("conv_embeddings", "id IS NOT NULL")
            .await?;
        Ok(())
    }

    /// Count total stored conversation embeddings.
    pub async fn count(&self) -> common::Result<usize> {
        Ok(self.vector_store.count("conv_embeddings").await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recall_config_defaults() {
        let config = RecallConfig::default();
        assert!((config.decay_half_life_days - 138.0).abs() < f64::EPSILON);
        assert!((config.default_threshold - 0.4).abs() < f32::EPSILON);
        assert_eq!(config.default_limit, 5);
    }

    #[test]
    fn test_decay_math() {
        let half_life = 138.0;
        let decay_factor = 0.5_f64.powf(1.0 / half_life);
        // At exactly half_life days, score should be halved
        let score_at_half_life = decay_factor.powf(half_life);
        assert!((score_at_half_life - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConversationRecallService>();
    }
}
