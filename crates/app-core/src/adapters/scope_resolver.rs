//! Concrete ScopeResolver — resolves related note IDs for insight context.
//!
//! Supports scope types:
//! - Backlinks (Linked): notes that wikilink to the target note
//! - Notebook: notes in the same notebook + all nested child notebooks (recursive)
//! - Project: alias for flat same-notebook (legacy)
//! - Semantic / Manual: kept for backwards compatibility

use async_trait::async_trait;
use feature_insights::{ScopeConfig, ScopeResolver, ScopeType};
use feature_notes::repo::NoteRepo;
use storage::VectorStore;

pub struct ScopeResolverImpl {
    note_repo: NoteRepo,
    vector_store: Option<VectorStore>,
}

impl ScopeResolverImpl {
    pub fn new(note_repo: NoteRepo, vector_store: Option<VectorStore>) -> Self {
        Self {
            note_repo,
            vector_store,
        }
    }

    /// Linked scope: notes that link TO this note (backlinks) + notes this
    /// note links FROM (outgoing). Both directions give a complete picture.
    async fn resolve_backlinks(&self, note_id: &str) -> Vec<String> {
        let (backlinks, outgoing) = tokio::join!(
            self.note_repo.get_backlinks_with_context(note_id),
            self.note_repo.get_links_from(note_id),
        );

        let mut ids = std::collections::HashSet::new();

        // Incoming: notes that link to this note
        for (note, _ctx) in backlinks.unwrap_or_default() {
            ids.insert(note.id);
        }

        // Outgoing: notes this note links to
        for link in outgoing.unwrap_or_default() {
            ids.insert(link.target_id);
        }

        let mut result: Vec<String> = ids.into_iter().collect();
        result.sort();
        result
    }

    async fn resolve_semantic(&self, note_id: &str, radius: f64) -> Vec<String> {
        let Some(ref vs) = self.vector_store else {
            tracing::warn!("semantic scope: vector_store is None");
            return Vec::new();
        };
        let embedding = match vs.get_embedding("note_embeddings", note_id).await {
            Ok(Some(v)) => {
                tracing::debug!(note_id, dim = v.len(), "semantic scope: got embedding");
                v
            }
            Ok(None) => {
                tracing::warn!(note_id, "semantic scope: no embedding found for note");
                return Vec::new();
            }
            Err(e) => {
                tracing::warn!(note_id, error = %e, "semantic scope: get_embedding failed");
                return Vec::new();
            }
        };
        match vs
            .search_similar("note_embeddings", &embedding, 20, radius)
            .await
        {
            Ok(results) => {
                tracing::info!(note_id, count = results.len(), radius, "semantic scope: search results");
                results
                    .into_iter()
                    .map(|(id, _score)| id)
                    .filter(|id| id != note_id)
                    .collect()
            }
            Err(e) => {
                tracing::warn!(note_id, error = %e, "semantic scope: search_similar failed");
                Vec::new()
            }
        }
    }

    async fn resolve_project(&self, note_id: &str) -> Vec<String> {
        let note = match self.note_repo.get_note(note_id).await {
            Ok(Some(n)) => n,
            _ => return Vec::new(),
        };
        let Some(ref notebook_id) = note.notebook_id else {
            return Vec::new();
        };
        match self.note_repo.list_notes(Some(notebook_id)).await {
            Ok(notes) => notes
                .into_iter()
                .map(|n| n.id)
                .filter(|id| id != note_id)
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Notebook scope: get all notes in the same notebook AND all nested
    /// child notebooks (recursive). This is the deep version of "project".
    async fn resolve_notebook(&self, note_id: &str) -> Vec<String> {
        let note = match self.note_repo.get_note(note_id).await {
            Ok(Some(n)) => n,
            _ => return Vec::new(),
        };
        let Some(ref root_notebook_id) = note.notebook_id else {
            return Vec::new();
        };

        // Collect all notebook IDs in the tree (root + descendants)
        let all_notebooks = match self.note_repo.list_notebooks().await {
            Ok(nbs) => nbs,
            Err(_) => return Vec::new(),
        };

        let mut notebook_ids = std::collections::HashSet::new();
        notebook_ids.insert(root_notebook_id.clone());

        // BFS to find all descendant notebooks
        let mut queue = vec![root_notebook_id.clone()];
        while let Some(parent) = queue.pop() {
            for nb in &all_notebooks {
                if nb.parent_id.as_deref() == Some(&parent) && notebook_ids.insert(nb.id.clone()) {
                    queue.push(nb.id.clone());
                }
            }
        }

        // Collect notes from all notebooks in the tree
        let mut note_ids = Vec::new();
        for nb_id in &notebook_ids {
            if let Ok(notes) = self.note_repo.list_notes(Some(nb_id)).await {
                for n in notes {
                    if n.id != note_id {
                        note_ids.push(n.id);
                    }
                }
            }
        }
        note_ids.sort();
        note_ids.dedup();
        note_ids
    }
}

#[async_trait]
impl ScopeResolver for ScopeResolverImpl {
    async fn resolve(&self, note_id: &str, config: &ScopeConfig) -> Vec<String> {
        match config.scope_type {
            ScopeType::Backlinks => self.resolve_backlinks(note_id).await,
            ScopeType::Semantic => self.resolve_semantic(note_id, config.radius).await,
            ScopeType::Project => self.resolve_project(note_id).await,
            ScopeType::Manual => config.node_ids.clone(),
            ScopeType::Notebook => self.resolve_notebook(note_id).await,
        }
    }
}
