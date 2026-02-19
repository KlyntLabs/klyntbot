//! Memory system for agent persistence (SQL-backed).

use chrono::{Datelike, Utc};
use tracing::{debug, warn};

use common::Result;

/// Memory store for daily notes and long-term memory, backed by PostgreSQL.
pub struct MemoryStore {
    repo: storage::MemoryNoteRepo,
}

impl MemoryStore {
    /// Create a SQL-backed memory store.
    pub fn new(repo: storage::MemoryNoteRepo) -> Self {
        Self { repo }
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

        self.repo
            .append(&key, content)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        debug!("Appended to today's memory note (SQL): {}", key);
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
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        debug!("Updated long-term memory (SQL)");
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
}
