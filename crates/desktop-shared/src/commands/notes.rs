use serde::{Deserialize, Serialize};

// ── Notes ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteResponse {
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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCreateParams {
    pub title: String,
    pub notebook_id: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub pinned: Option<bool>,
    /// `None` = don't change, `Some(None)` = move to root, `Some(Some(id))` = move to folder
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub notebook_id: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    /// `None` = don't change, `Some(None)` = clear icon, `Some(Some(emoji))` = set icon
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub icon: Option<Option<String>>,
    /// `None` = don't change, `Some(None)` = clear color, `Some(Some(hex))` = set color
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub color: Option<Option<String>>,
}

/// Deserializes a field that distinguishes between absent, null, and present.
/// - absent → `None` (don't change)
/// - `null` → `Some(None)` (set to null / move to root)
/// - `"value"` → `Some(Some("value"))` (set to value)
fn deserialize_nullable_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub parent_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub note_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteLinkResponse {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersionResponse {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookCreateParams {
    pub title: String,
    pub parent_id: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

// ── Inbox ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxCreateParams {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemResponse {
    pub id: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

// ── Backlinks ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkResponse {
    pub note: NoteResponse,
    pub context: Option<String>,
}
