//! Memory system for agent persistence.

use chrono::{Datelike, Utc};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, warn};

use klyntbot_core::Result;

/// Memory store for daily notes and long-term memory
pub struct MemoryStore {
    memory_dir: PathBuf,
}

impl MemoryStore {
    /// Create a new memory store
    pub fn new(workspace_path: PathBuf) -> Self {
        let memory_dir = workspace_path.join("memory");

        // Create memory directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&memory_dir) {
            warn!("Failed to create memory directory: {}", e);
        }

        Self { memory_dir }
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
        let filename = format!(
            "{:04}-{:02}-{:02}.md",
            today.year(),
            today.month(),
            today.day()
        );
        let path = self.memory_dir.join(filename);

        if path.exists() {
            Ok(fs::read_to_string(&path).await?)
        } else {
            Ok(String::new())
        }
    }

    /// Append to today's daily note
    pub async fn append_today(&self, content: &str) -> Result<()> {
        let today = Utc::now();
        let filename = format!(
            "{:04}-{:02}-{:02}.md",
            today.year(),
            today.month(),
            today.day()
        );
        let path = self.memory_dir.join(filename);

        let existing = if path.exists() {
            fs::read_to_string(&path).await?
        } else {
            String::new()
        };

        let new_content = if existing.is_empty() {
            content.to_string()
        } else {
            format!("{}\n\n{}", existing, content)
        };

        fs::write(&path, new_content).await?;
        debug!("Appended to today's memory: {}", path.display());

        Ok(())
    }

    /// Read long-term memory
    pub async fn read_long_term(&self) -> Result<String> {
        let path = self.memory_dir.join("MEMORY.md");

        if path.exists() {
            Ok(fs::read_to_string(&path).await?)
        } else {
            Ok(String::new())
        }
    }

    /// Write long-term memory
    pub async fn write_long_term(&self, content: &str) -> Result<()> {
        let path = self.memory_dir.join("MEMORY.md");
        fs::write(&path, content).await?;
        debug!("Updated long-term memory: {}", path.display());
        Ok(())
    }

    /// Get recent memories from the last N days
    pub async fn get_recent_memories(&self, days: usize) -> Result<Vec<(String, String)>> {
        let mut memories = Vec::new();

        // Get all .md files in memory directory
        let mut entries = fs::read_dir(&self.memory_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    // Skip MEMORY.md
                    if filename == "MEMORY" {
                        continue;
                    }

                    // Check if it's a date file (YYYY-MM-DD format)
                    if filename.len() == 10 && filename.chars().filter(|c| *c == '-').count() == 2 {
                        if let Ok(content) = fs::read_to_string(&path).await {
                            memories.push((filename.to_string(), content));
                        }
                    }
                }
            }
        }

        // Sort by date (newest first)
        memories.sort_by(|a, b| b.0.cmp(&a.0));

        // Take only the last N days
        memories.truncate(days);

        Ok(memories)
    }

    /// List all memory files
    pub async fn list_memory_files(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();

        let mut entries = fs::read_dir(&self.memory_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    files.push(filename.to_string());
                }
            }
        }

        files.sort();
        files.reverse(); // Newest first

        Ok(files)
    }
}
