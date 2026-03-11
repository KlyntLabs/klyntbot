//! Content registry types for multi-source documentation and skills.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where a content source originates from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentSourceKind {
    /// Built into the klyntbot binary.
    Builtin,
    /// Loaded from a local filesystem directory.
    Local { name: String, path: PathBuf },
    /// Fetched from a remote URL with local caching.
    Remote {
        name: String,
        url: String,
        cache_dir: PathBuf,
    },
}

/// A documentation entry in the content registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub tags: Vec<String>,
    pub content_source: String,
    pub languages: Vec<LanguageEntry>,
}

/// Language/version info associated with a doc entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageEntry {
    pub language: String,
    pub recommended_version: String,
}

/// A skill entry in the content registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub tags: Vec<String>,
    pub content_source: String,
    pub path: PathBuf,
    pub files: Vec<String>,
}

/// A unified content entry — either a doc or a skill.
#[derive(Debug, Clone)]
pub enum ContentEntry {
    Doc(DocEntry),
    Skill(SkillEntry),
}

/// A search result with relevance score.
#[derive(Debug, Clone)]
pub struct ContentSearchResult {
    pub entry: ContentEntry,
    pub score: f64,
}
