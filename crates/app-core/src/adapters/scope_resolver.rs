//! Concrete ScopeResolver — resolves related note IDs for insight context.
//!
//! Supports 4 scope types:
//! - Backlinks: notes that wikilink to the target note
//! - Semantic: notes with similar embeddings (cosine similarity ≥ radius)
//! - Project: notes in the same notebook
//! - Manual: user-specified note IDs

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

    async fn resolve_backlinks(&self, note_id: &str) -> Vec<String> {
        let backlinks = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default();
        let mut ids: Vec<String> = backlinks.into_iter().map(|(note, _ctx)| note.id).collect();
        ids.sort();
        ids
    }

    async fn resolve_semantic(&self, note_id: &str, radius: f64) -> Vec<String> {
        let Some(ref vs) = self.vector_store else {
            return Vec::new();
        };
        // Fetch the note's own embedding vector
        let embedding = match vs.get_embedding("note_embeddings", note_id).await {
            Ok(Some(v)) => v,
            _ => return Vec::new(),
        };
        // Search for similar notes using cosine similarity
        match vs
            .search_similar("note_embeddings", &embedding, 20, radius)
            .await
        {
            Ok(results) => results
                .into_iter()
                .map(|(id, _score)| id)
                .filter(|id| id != note_id)
                .collect(),
            Err(_) => Vec::new(),
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
}

#[async_trait]
impl ScopeResolver for ScopeResolverImpl {
    async fn resolve(&self, note_id: &str, config: &ScopeConfig) -> Vec<String> {
        match config.scope_type {
            ScopeType::Backlinks => self.resolve_backlinks(note_id).await,
            ScopeType::Semantic => self.resolve_semantic(note_id, config.radius).await,
            ScopeType::Project => self.resolve_project(note_id).await,
            ScopeType::Manual => config.node_ids.clone(),
        }
    }
}
