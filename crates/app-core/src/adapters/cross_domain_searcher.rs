//! Concrete CrossDomainSearcher — searches LanceDB embedding tables across
//! domains and looks up entity metadata from SQLite.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use feature_insights::cross_domain::{EntityDomain, EntityRef};
use feature_insights::traits::{CrossDomainSearcher, CrossDomainVectorHit};
use feature_notes::repo::NoteRepo;
use storage::TaskRepo;
use storage::VectorStore;
use tools::embedding_engine::EmbeddingEngine;
use tracing::{debug, warn};

/// Maps each `EntityDomain` to its LanceDB embedding table name.
///
/// Returns `None` for domains that don't have a dedicated embedding table yet
/// (e.g. Finance uses `tree_node_embeddings` which has a different schema).
fn table_for_domain(domain: &EntityDomain) -> Option<&'static str> {
    match domain {
        EntityDomain::Task => Some("task_embeddings"),
        EntityDomain::Note => Some("note_embeddings"),
        // Finance tree node embeddings have a different schema (note_id, level,
        // source_type) and would need special handling. Skipped for now.
        EntityDomain::Finance | EntityDomain::Productivity => None,
    }
}

/// All domains we can search as targets.
const SEARCHABLE_DOMAINS: &[EntityDomain] = &[EntityDomain::Task, EntityDomain::Note];

/// LanceDB similarity threshold for cross-domain search.
const SEARCH_THRESHOLD: f64 = 0.72;

/// Maximum hits per target domain table.
const SEARCH_LIMIT: usize = 3;

pub struct CrossDomainSearcherImpl {
    vector_store: VectorStore,
    embedding_engine: Arc<EmbeddingEngine>,
    task_repo: TaskRepo,
    note_repo: NoteRepo,
}

impl CrossDomainSearcherImpl {
    pub fn new(
        vector_store: VectorStore,
        embedding_engine: Arc<EmbeddingEngine>,
        task_repo: TaskRepo,
        note_repo: NoteRepo,
    ) -> Self {
        Self {
            vector_store,
            embedding_engine,
            task_repo,
            note_repo,
        }
    }

    /// Try to get the source entity's existing embedding from its domain table,
    /// falling back to generating one from the title text.
    async fn get_or_generate_embedding(
        &self,
        source_domain: &EntityDomain,
        source_id: &str,
        source_title: &str,
    ) -> Option<Vec<f32>> {
        // Try the existing embedding first (cheaper, already computed on create).
        if let Some(table) = table_for_domain(source_domain) {
            match self.vector_store.get_embedding(table, source_id).await {
                Ok(Some(vec)) => return Some(vec),
                Ok(None) => {
                    debug!(
                        source_id,
                        table, "no existing embedding found, will generate from title"
                    );
                }
                Err(e) => {
                    warn!(source_id, table, error = %e, "failed to fetch source embedding");
                }
            }
        }

        // Fallback: generate embedding from title text.
        match self
            .embedding_engine
            .clone()
            .embed_async(source_title.to_string())
            .await
        {
            Ok(vec) => Some(vec),
            Err(e) => {
                warn!(source_id, error = %e, "failed to generate embedding from title");
                None
            }
        }
    }

    /// Look up task metadata by ID. Returns (title, created_at) if found.
    async fn lookup_task(&self, id: &str) -> Option<(String, DateTime<Utc>)> {
        match self.task_repo.get(id).await {
            Ok(Some(row)) => Some((row.title, row.created_at)),
            Ok(None) => {
                debug!(id, "task not found for cross-domain hit");
                None
            }
            Err(e) => {
                warn!(id, error = %e, "failed to look up task for cross-domain hit");
                None
            }
        }
    }

    /// Look up note metadata by ID. Returns (title, created_at) if found.
    async fn lookup_note(&self, id: &str) -> Option<(String, DateTime<Utc>)> {
        match self.note_repo.get_note(id).await {
            Ok(Some(row)) => {
                let created_at = row
                    .created_at
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_else(|_| Utc::now());
                Some((row.title, created_at))
            }
            Ok(None) => {
                debug!(id, "note not found for cross-domain hit");
                None
            }
            Err(e) => {
                warn!(id, error = %e, "failed to look up note for cross-domain hit");
                None
            }
        }
    }

    /// Look up entity metadata for a vector hit in a target domain.
    async fn lookup_entity(
        &self,
        domain: &EntityDomain,
        id: &str,
    ) -> Option<(String, DateTime<Utc>)> {
        match domain {
            EntityDomain::Task => self.lookup_task(id).await,
            EntityDomain::Note => self.lookup_note(id).await,
            _ => None,
        }
    }
}

#[async_trait]
impl CrossDomainSearcher for CrossDomainSearcherImpl {
    async fn search_other_domains(
        &self,
        source_domain: &EntityDomain,
        source_id: &str,
        source_title: &str,
    ) -> Vec<CrossDomainVectorHit> {
        let query_vec = match self
            .get_or_generate_embedding(source_domain, source_id, source_title)
            .await
        {
            Some(v) => v,
            None => return Vec::new(),
        };

        let mut results = Vec::new();

        for target_domain in SEARCHABLE_DOMAINS {
            // Skip same-domain hits (we only want cross-domain connections).
            if target_domain == source_domain {
                continue;
            }

            let table = match table_for_domain(target_domain) {
                Some(t) => t,
                None => continue,
            };

            let hits = match self
                .vector_store
                .search_similar(table, &query_vec, SEARCH_LIMIT, SEARCH_THRESHOLD)
                .await
            {
                Ok(h) => h,
                Err(e) => {
                    warn!(table, error = %e, "cross-domain vector search failed");
                    continue;
                }
            };

            for (hit_id, cosine) in hits {
                // Skip if this is somehow the same entity ID.
                if hit_id == source_id {
                    continue;
                }

                if let Some((title, created_at)) =
                    self.lookup_entity(target_domain, &hit_id).await
                {
                    results.push(CrossDomainVectorHit {
                        entity: EntityRef {
                            domain: target_domain.clone(),
                            id: hit_id,
                            title,
                        },
                        cosine,
                        created_at,
                    });
                }
            }
        }

        debug!(
            source_domain = ?source_domain,
            source_id,
            hits = results.len(),
            "cross-domain vector search complete"
        );

        results
    }
}
