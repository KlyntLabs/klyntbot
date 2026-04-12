//! DatabaseContextSource — injects active entities from all flexible databases.

use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use context_engine::TtlCache;
use tracing::warn;

const DATABASE_CACHE_TTL_SECS: i64 = 60;

/// Provides a summary of active entities across all user databases.
pub struct DatabaseContextSource {
    entity_store: Arc<entity_store::store::EntityStore>,
    cache: TtlCache,
}

impl DatabaseContextSource {
    pub fn new(entity_store: Arc<entity_store::store::EntityStore>) -> Self {
        Self {
            entity_store,
            cache: TtlCache::new(DATABASE_CACHE_TTL_SECS),
        }
    }

    async fn build_context(&self) -> String {
        let databases = match self.entity_store.list_databases().await {
            Ok(dbs) => dbs,
            Err(e) => {
                warn!("DatabaseContextSource: failed to list databases: {e}");
                return String::new();
            }
        };

        if databases.is_empty() {
            return String::new();
        }

        let mut sections = Vec::new();

        for db in &databases {
            let params = entity_store::query::QueryParams {
                filters: vec![],
                sorts: vec![entity_store::SortRule {
                    field: "created_at".into(),
                    direction: entity_store::SortDirection::Desc,
                }],
                limit: Some(10),
                offset: None,
            };

            match entity_store::query::query_entities(self.entity_store.pool(), db, &params).await {
                Ok(result) if !result.entities.is_empty() => {
                    let mut section = format!("### {} ({} entities)\n", db.name, result.total);
                    for entity in &result.entities {
                        let formatted =
                            entity_store::format_entity_fields(&entity.fields, Some(&db.fields));
                        section.push_str(&format!("- {formatted}\n"));
                    }
                    sections.push(section);
                }
                Ok(_) => {} // empty database, skip
                Err(e) => {
                    warn!("DatabaseContextSource: failed to query {}: {e}", db.name);
                }
            }
        }

        if sections.is_empty() {
            return String::new();
        }

        format!("## Active Databases\n\n{}", sections.join("\n"))
    }
}

#[async_trait]
impl ContextSource for DatabaseContextSource {
    fn name(&self) -> &str {
        "databases"
    }

    fn priority(&self) -> u8 {
        65
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        if let Some(cached) = self.cache.get() {
            return if cached.trim().is_empty() {
                None
            } else {
                Some(cached)
            };
        }

        let content = self.build_context().await;
        let result = if content.trim().is_empty() {
            None
        } else {
            Some(content.clone())
        };

        self.cache.set(content);
        result
    }

    fn estimated_tokens(&self) -> usize {
        800
    }
}
