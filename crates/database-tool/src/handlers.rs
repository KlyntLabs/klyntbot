//! Generic handler traits for future AI-powered operations.

use async_trait::async_trait;
use common::Result;
use entity_store::{DatabaseSchema, Entity};
use std::collections::HashMap;

/// Enrichment handler — auto-fills entity fields via AI (e.g. summarise, categorise).
#[async_trait]
pub trait EntityEnrichmentHandler: Send + Sync {
    async fn enrich(
        &self,
        entity: &Entity,
        schema: &DatabaseSchema,
    ) -> Result<Option<HashMap<String, serde_json::Value>>>;
}

/// Embedding handler — indexes entities for semantic search.
#[async_trait]
pub trait EntityEmbeddingHandler: Send + Sync {
    async fn embed_entity(&self, entity: &Entity, schema: &DatabaseSchema) -> Result<()>;
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
}
