//! DatabaseEmbeddingAdapter — embeds entities from flexible databases into LanceDB.

use std::collections::HashMap;
use std::sync::Arc;

use cognitive::TextEmbedder;
use tracing::debug;

/// Embeds entities from any database into per-database LanceDB embedding tables.
pub struct DatabaseEmbeddingAdapter {
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
}

impl DatabaseEmbeddingAdapter {
    pub fn new(vector_store: Arc<storage::VectorStore>, embedder: Arc<dyn TextEmbedder>) -> Self {
        Self {
            vector_store,
            embedder,
        }
    }

    /// Compose embedding text from entity fields.
    pub fn compose_text(
        database_name: &str,
        fields: &HashMap<String, serde_json::Value>,
    ) -> String {
        let mut parts = Vec::new();
        for (key, value) in fields {
            let val_str = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => continue,
                other => other.to_string(),
            };
            if !val_str.is_empty() {
                parts.push(format!("{key}={val_str}"));
            }
        }
        format!("{database_name} entity: {}", parts.join(". "))
    }

    /// Embed a single entity from a database.
    pub async fn embed_entity(
        &self,
        database_id: &str,
        database_name: &str,
        entity_id: &str,
        fields: &HashMap<String, serde_json::Value>,
    ) -> common::Result<()> {
        let text = Self::compose_text(database_name, fields);
        let embedding = self.embedder.embed(&text).await?;
        let table_name = format!("db_{database_id}_embeddings");

        self.vector_store
            .upsert_database_entity_embedding(
                &table_name,
                entity_id,
                &embedding,
                database_id,
                &text,
            )
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        debug!(entity_id = %entity_id, database_id = %database_id, "Database entity embedding upserted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_text_formats_correctly() {
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("Fix bug"));
        fields.insert("status".to_string(), serde_json::json!("doing"));
        fields.insert("empty".to_string(), serde_json::Value::Null);

        let text = DatabaseEmbeddingAdapter::compose_text("Tasks", &fields);
        assert!(text.starts_with("Tasks entity: "));
        assert!(text.contains("title=Fix bug"));
        assert!(text.contains("status=doing"));
        assert!(!text.contains("empty"));
    }
}
