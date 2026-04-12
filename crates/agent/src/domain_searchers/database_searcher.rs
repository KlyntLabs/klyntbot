//! DatabaseSearcher — keyword search across all flexible databases.

use std::sync::Arc;

use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use tracing::debug;

pub struct DatabaseSearcher {
    entity_store: Arc<entity_store::store::EntityStore>,
}

impl DatabaseSearcher {
    pub fn new(entity_store: Arc<entity_store::store::EntityStore>) -> Self {
        Self { entity_store }
    }

    fn extract_search_term(query: &str) -> String {
        super::extract_first_keyword(
            query,
            &[
                "database",
                "databases",
                "entities",
                "entity",
                "items",
                "records",
            ],
        )
    }
}

#[async_trait]
impl DomainSearcher for DatabaseSearcher {
    fn domain_name(&self) -> &str {
        "databases"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let search_term = Self::extract_search_term(query);
        if search_term.is_empty() {
            return Vec::new();
        }

        debug!(
            original_query = query,
            search_term = search_term.as_str(),
            "DatabaseSearcher: searching"
        );

        let databases = match self.entity_store.list_databases().await {
            Ok(dbs) => dbs,
            Err(e) => {
                debug!(error = %e, "DatabaseSearcher: failed to list databases");
                return Vec::new();
            }
        };

        let mut all_results: Vec<(String, entity_store::types::Entity)> = Vec::new();
        for db in &databases {
            match self
                .entity_store
                .search_entities(&db.id, &search_term, limit)
                .await
            {
                Ok(entities) => {
                    for entity in entities {
                        all_results.push((db.name.clone(), entity));
                    }
                }
                Err(e) => {
                    debug!(error = %e, db_name = db.name.as_str(), "DatabaseSearcher: search error for database");
                }
            }
            if all_results.len() >= limit {
                break;
            }
        }

        all_results.truncate(limit);

        debug!(result_count = all_results.len(), "DatabaseSearcher: found");

        all_results
            .into_iter()
            .enumerate()
            .map(|(i, (db_name, entity))| {
                let formatted = entity_store::format_entity_fields(&entity.fields, None);

                MemoryEntry {
                    id: entity.id.clone(),
                    content: format!("[{db_name}] {formatted}"),
                    score: 1.0 / (1.0 + i as f64),
                    source: MemorySource::Domain {
                        name: "databases".into(),
                    },
                    raw_score: 0.0,
                }
            })
            .collect()
    }
}
