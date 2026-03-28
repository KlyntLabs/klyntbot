//! Domain models and SQLite row types for notes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersion {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteLink {
    pub source_id: String,
    pub target_id: String,
}

/// SQLite row for notebooks (maps 1:1 to table).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotebookRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// SQLite row for notes (maps 1:1 to table).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteRow {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: Option<String>,
    pub body_json: Option<String>,
    pub pinned: i32,
    pub archived: i32,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub embedding_updated_at: Option<String>,
    pub split_content: Option<String>,
    pub split_mode: Option<String>,
    pub perspective_config: Option<String>,
    pub last_visited_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// FTS5 search result with BM25 relevance rank.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteSearchResult {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub body_json: Option<String>,
    pub pinned: i32,
    pub archived: i32,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub embedding_updated_at: Option<String>,
    pub split_content: Option<String>,
    pub split_mode: Option<String>,
    pub perspective_config: Option<String>,
    pub last_visited_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub rank: f64,
}

/// SQLite row for inbox items.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InboxItemRow {
    pub id: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

/// SQLite row for note versions.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteVersionRow {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: String,
}

/// SQLite row for note tags.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteTagRow {
    pub note_id: String,
    pub tag: String,
}

/// SQLite row for note links.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteLinkRow {
    pub source_id: String,
    pub target_id: String,
}

// ── Row → Domain conversions ────────────────────────────────────────────

impl From<NotebookRow> for Notebook {
    fn from(r: NotebookRow) -> Self {
        Self {
            id: r.id,
            parent_id: r.parent_id,
            title: r.title,
            icon: r.icon,
            color: r.color,
            sort_order: r.sort_order,
            created_at: r.created_at.parse().unwrap_or_else(|e| {
                tracing::warn!(raw = %r.created_at, error = %e, "failed to parse notebook created_at");
                Default::default()
            }),
            updated_at: r.updated_at.parse().unwrap_or_else(|e| {
                tracing::warn!(raw = %r.updated_at, error = %e, "failed to parse notebook updated_at");
                Default::default()
            }),
        }
    }
}

impl Note {
    /// Create a Note from a database row with its associated tags.
    /// Prefer this over `From<NoteRow>` which leaves tags empty.
    pub fn from_row(row: NoteRow, tags: Vec<String>) -> Self {
        Self {
            id: row.id,
            notebook_id: row.notebook_id,
            title: row.title,
            body: row.body,
            body_html: row.body_html,
            pinned: row.pinned != 0,
            archived: row.archived != 0,
            icon: row.icon,
            color: row.color,
            tags,
            created_at: row.created_at.parse().unwrap_or_else(|e| {
                tracing::warn!(raw = %row.created_at, error = %e, "failed to parse note created_at");
                Default::default()
            }),
            updated_at: row.updated_at.parse().unwrap_or_else(|e| {
                tracing::warn!(raw = %row.updated_at, error = %e, "failed to parse note updated_at");
                Default::default()
            }),
        }
    }
}

/// Warning: tags are NOT populated (set to empty vec).
/// Use `Note::from_row(row, tags)` for complete conversion.
impl From<NoteRow> for Note {
    fn from(r: NoteRow) -> Self {
        Self::from_row(r, vec![])
    }
}

impl From<NoteVersionRow> for NoteVersion {
    fn from(r: NoteVersionRow) -> Self {
        Self {
            id: r.id,
            note_id: r.note_id,
            body: r.body,
            created_at: r.created_at.parse().unwrap_or_else(|e| {
                tracing::warn!(raw = %r.created_at, error = %e, "failed to parse version created_at");
                Default::default()
            }),
        }
    }
}
