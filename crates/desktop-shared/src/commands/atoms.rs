use serde::{Deserialize, Serialize};

// ── Params ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomsForNoteParams {
    pub note_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomAcceptParams {
    pub atom_id: String,
    pub personal_importance: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomDismissParams {
    pub atom_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomRestoreParams {
    pub atom_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomNextCardParams {
    pub atom_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomBulkAcceptParams {
    pub atom_ids: Vec<String>,
    pub personal_importance: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomMigrationStatusParams {}

// ── Responses ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeAtomResponse {
    pub id: String,
    pub subject: String,
    pub atom_type: String,
    pub domain: String,
    pub source_note_id: Option<String>,
    pub source_range: Option<String>,
    pub source_context: Option<String>,
    pub semantic_fact_id: Option<String>,
    pub retention_pct: f64,
    pub personal_importance: f64,
    pub status: String,
    pub salience: f64,
    pub last_interaction_ts: Option<String>,
    pub metadata: Option<String>,
    pub topic_name: Option<String>,
    pub linked_card_count: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AtomMigrationStatusResponse {
    pub migrated: bool,
    pub count: usize,
}
