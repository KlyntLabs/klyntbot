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
    pub pinned: i32,
    pub archived: i32,
    pub created_at: String,
    pub updated_at: String,
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
            sort_order: r.sort_order,
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl From<NoteRow> for Note {
    fn from(r: NoteRow) -> Self {
        Self {
            id: r.id,
            notebook_id: r.notebook_id,
            title: r.title,
            body: r.body,
            body_html: r.body_html,
            pinned: r.pinned != 0,
            archived: r.archived != 0,
            tags: vec![], // populated separately
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl From<NoteVersionRow> for NoteVersion {
    fn from(r: NoteVersionRow) -> Self {
        Self {
            id: r.id,
            note_id: r.note_id,
            body: r.body,
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}
