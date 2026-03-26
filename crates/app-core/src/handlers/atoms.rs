use desktop_shared::commands::KnowledgeAtomResponse;
use desktop_shared::errors::ApiError;

pub(crate) fn map_db(e: sqlx::Error) -> ApiError {
    ApiError::new("INTERNAL_ERROR", e.to_string())
}

pub(crate) fn atom_row_to_response(row: cognitive::KnowledgeAtomRow) -> KnowledgeAtomResponse {
    KnowledgeAtomResponse {
        id: row.id,
        subject: row.subject,
        atom_type: row.atom_type,
        domain: row.domain,
        source_note_id: row.source_note_id,
        source_range: row.source_range,
        source_context: row.source_context,
        semantic_fact_id: row.semantic_fact_id,
        retention_pct: row.retention_pct,
        personal_importance: row.personal_importance,
        status: row.status,
        salience: row.salience,
        last_interaction_ts: row.last_interaction_ts,
        metadata: row.metadata,
        topic_name: None,
        linked_card_count: 0,
        created_at: row.created_at,
    }
}
