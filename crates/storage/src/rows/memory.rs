//! Row struct for the `memory_notes` table.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// Row struct for the `memory_notes` table.
///
/// Keys are either date strings (`"2026-02-19"`) for daily notes
/// or `"LONG_TERM"` for persistent memory.
#[derive(Debug, Clone, FromRow)]
pub struct MemoryNoteRow {
    pub note_key: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
