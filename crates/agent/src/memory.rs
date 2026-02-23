//! Memory system for agent persistence (SQL-backed).

use std::sync::Arc;

use chrono::{Datelike, Utc};
use tracing::{debug, warn};

use common::Result;
use tools::embedding_engine::EmbeddingEngine;

/// Memory store for daily notes and long-term memory, backed by SQLite + LanceDB.
pub struct MemoryStore {
    repo: storage::MemoryNoteRepo,
    embedding_store: Option<storage::VectorStore>,
    embedding_engine: Option<Arc<EmbeddingEngine>>,
    similarity_threshold: f64,
}

impl MemoryStore {
    /// Create a SQL-backed memory store (no embedding support).
    pub fn new(repo: storage::MemoryNoteRepo) -> Self {
        Self {
            repo,
            embedding_store: None,
            embedding_engine: None,
            similarity_threshold: 0.5,
        }
    }

    /// Create a memory store with embedding-based relevance filtering.
    pub fn with_embeddings(
        repo: storage::MemoryNoteRepo,
        embedding_store: storage::VectorStore,
        embedding_engine: Arc<EmbeddingEngine>,
        similarity_threshold: f64,
    ) -> Self {
        Self {
            repo,
            embedding_store: Some(embedding_store),
            embedding_engine: Some(embedding_engine),
            similarity_threshold,
        }
    }

    /// Get memory context filtered by relevance to the query.
    /// Falls back to `get_memory_context()` if embeddings are unavailable.
    pub async fn get_relevant_memory(&self, query: &str, limit: usize) -> String {
        if let (Some(engine), Some(vs)) = (&self.embedding_engine, &self.embedding_store) {
            // Embed query (CPU-bound, run in blocking thread)
            let engine = Arc::clone(engine);
            let query_text = query.to_string();
            let embed_result = tokio::task::spawn_blocking(move || engine.embed(&query_text)).await;

            if let Ok(Ok(query_vec)) = embed_result {
                match vs
                    .search_similar(
                        "memory_note_embeddings",
                        &query_vec,
                        limit,
                        self.similarity_threshold,
                    )
                    .await
                {
                    Ok(matches) if !matches.is_empty() => {
                        let mut context = String::new();
                        context.push_str("# Relevant Memory\n\n");
                        for (note_key, similarity) in &matches {
                            if let Ok(Some(row)) = self.repo.get(note_key).await {
                                context.push_str(&format!(
                                    "## {} (relevance: {:.0}%)\n{}\n\n",
                                    note_key,
                                    similarity * 100.0,
                                    row.content
                                ));
                            }
                        }
                        return context;
                    }
                    Ok(_) => {} // No matches above threshold — fall through
                    Err(e) => {
                        warn!("Memory embedding search failed: {}", e);
                    }
                }
            }
        }

        // Fallback to dump-everything
        self.get_memory_context().await
    }

    /// Get memory context for system prompt (long-term + today's notes)
    pub async fn get_memory_context(&self) -> String {
        let mut context = String::new();

        // Read long-term memory
        if let Ok(long_term) = self.read_long_term().await {
            if !long_term.trim().is_empty() {
                context.push_str("# Long-term Memory\n\n");
                context.push_str(&long_term);
                context.push_str("\n\n");
            }
        }

        // Read today's notes
        if let Ok(today) = self.read_today().await {
            if !today.trim().is_empty() {
                context.push_str("# Today's Notes\n\n");
                context.push_str(&today);
            }
        }

        context
    }

    /// Read today's daily note
    pub async fn read_today(&self) -> Result<String> {
        let today = Utc::now();
        let key = format!(
            "{:04}-{:02}-{:02}",
            today.year(),
            today.month(),
            today.day()
        );

        match self.repo.get(&key).await {
            Ok(Some(row)) => Ok(row.content),
            Ok(None) => Ok(String::new()),
            Err(e) => {
                warn!("Failed to read today's note from SQL: {}", e);
                Ok(String::new())
            }
        }
    }

    /// Append to today's daily note
    pub async fn append_today(&self, content: &str) -> Result<()> {
        let today = Utc::now();
        let key = format!(
            "{:04}-{:02}-{:02}",
            today.year(),
            today.month(),
            today.day()
        );

        self.repo.append(&key, content).await?;
        debug!("Appended to today's memory note (SQL): {}", key);

        // Best-effort: embed the note for future relevance search
        self.embed_note(&key, content);

        Ok(())
    }

    /// Read long-term memory
    pub async fn read_long_term(&self) -> Result<String> {
        match self
            .repo
            .get(storage::repos::memory_note::LONG_TERM_KEY)
            .await
        {
            Ok(Some(row)) => Ok(row.content),
            Ok(None) => Ok(String::new()),
            Err(e) => {
                warn!("Failed to read long-term memory from SQL: {}", e);
                Ok(String::new())
            }
        }
    }

    /// Write long-term memory
    pub async fn write_long_term(&self, content: &str) -> Result<()> {
        self.repo
            .upsert(storage::repos::memory_note::LONG_TERM_KEY, content)
            .await?;
        debug!("Updated long-term memory (SQL)");

        // Best-effort: embed for future relevance search
        self.embed_note(storage::repos::memory_note::LONG_TERM_KEY, content);

        Ok(())
    }

    /// Get the N most recent memory entries (ordered newest-first, excludes LONG_TERM).
    pub async fn get_recent_memories(&self, limit: usize) -> Result<Vec<(String, String)>> {
        match self.repo.list_recent(limit as i64).await {
            Ok(rows) => Ok(rows.into_iter().map(|r| (r.note_key, r.content)).collect()),
            Err(e) => {
                warn!("Failed to list recent memories from SQL: {}", e);
                Ok(Vec::new())
            }
        }
    }

    /// List all memory keys
    pub async fn list_memory_files(&self) -> Result<Vec<String>> {
        match self.repo.list_keys().await {
            Ok(keys) => Ok(keys),
            Err(e) => {
                warn!("Failed to list memory keys from SQL: {}", e);
                Ok(Vec::new())
            }
        }
    }

    /// Fire-and-forget embed a memory note for semantic retrieval.
    fn embed_note(&self, key: &str, content: &str) {
        if let (Some(engine), Some(vs)) = (&self.embedding_engine, &self.embedding_store) {
            let engine = Arc::clone(engine);
            let vs = vs.clone();
            let key = key.to_string();
            let content = content.to_string();
            tokio::spawn(async move {
                let embed_result =
                    tokio::task::spawn_blocking(move || engine.embed(&content)).await;
                match embed_result {
                    Ok(Ok(vec)) => {
                        if let Err(e) = vs
                            .upsert_embedding("memory_note_embeddings", &key, &vec, &[])
                            .await
                        {
                            warn!("Failed to upsert memory note embedding: {}", e);
                        }
                    }
                    Ok(Err(e)) => {
                        warn!("Failed to embed memory note: {}", e);
                    }
                    Err(e) => {
                        warn!("Embedding task panicked: {}", e);
                    }
                }
            });
        }
    }
}
