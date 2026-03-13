use desktop_shared::commands::{NoteResponse, NoteVersionResponse, NotebookResponse};
use desktop_shared::errors::ApiError;
use feature_notes::link_parser;
use feature_notes::models::{NoteRow, NoteVersionRow, NotebookRow};

use crate::errors::map_storage_err;
use crate::state::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

pub(crate) fn note_row_to_response(row: &NoteRow, tags: Vec<String>) -> NoteResponse {
    NoteResponse {
        id: row.id.clone(),
        notebook_id: row.notebook_id.clone(),
        title: row.title.clone(),
        body: row.body.clone(),
        body_html: row.body_html.clone(),
        pinned: row.pinned != 0,
        archived: row.archived != 0,
        tags,
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}

pub(crate) async fn note_with_tags(
    core: &AppCore,
    row: &NoteRow,
) -> Result<NoteResponse, ApiError> {
    let tags = core
        .note_repo
        .get_tags(&row.id)
        .await
        .map_err(map_storage_err)?;
    Ok(note_row_to_response(row, tags))
}

pub(crate) fn notebook_row_to_response(row: &NotebookRow, note_count: i64) -> NotebookResponse {
    NotebookResponse {
        id: row.id.clone(),
        parent_id: row.parent_id.clone(),
        title: row.title.clone(),
        icon: row.icon.clone(),
        sort_order: row.sort_order,
        note_count,
    }
}

/// Convert a list of NoteRows to NoteResponses with batch-fetched tags (single query).
pub(crate) async fn notes_with_tags_batch(
    core: &AppCore,
    rows: &[NoteRow],
) -> Result<Vec<NoteResponse>, ApiError> {
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let mut tag_map = core
        .note_repo
        .get_tags_batch(&ids)
        .await
        .map_err(map_storage_err)?;
    Ok(rows
        .iter()
        .map(|row| {
            let tags = tag_map.remove(&row.id).unwrap_or_default();
            note_row_to_response(row, tags)
        })
        .collect())
}

pub(crate) fn version_row_to_response(row: &NoteVersionRow) -> NoteVersionResponse {
    NoteVersionResponse {
        id: row.id.clone(),
        note_id: row.note_id.clone(),
        body: row.body.clone(),
        created_at: row.created_at.clone(),
    }
}

/// Extract wiki-links and entity mentions from note content, updating the
/// `note_links` and `note_entity_mentions` tables.
/// Always writes to DB (even with empty vecs) to clear stale rows.
pub(crate) async fn extract_links_and_mentions(
    core: &AppCore,
    note_id: &str,
    row: &NoteRow,
) -> Result<(), ApiError> {
    // 1. Extract link targets from HTML (data-note-id attributes)
    let mut target_ids: Vec<String> = Vec::new();
    if let Some(html) = &row.body_html {
        target_ids.extend(link_parser::extract_wiki_link_ids(html));
    }

    // 2. Also resolve [[Title]] from plain text body
    let titles = link_parser::extract_wiki_link_titles(&row.body);
    if !titles.is_empty() {
        let resolved = core
            .note_repo
            .resolve_titles_to_ids(&titles)
            .await
            .map_err(map_storage_err)?;
        for (_title, id) in resolved {
            if !target_ids.contains(&id) {
                target_ids.push(id);
            }
        }
    }

    // Filter out self-links
    target_ids.retain(|id| id != note_id);

    // 3. Extract entity mentions (@task:id, @project:id)
    let mentions = link_parser::extract_entity_mentions(&row.body);

    core.note_repo
        .set_links(note_id, &target_ids)
        .await
        .map_err(map_storage_err)?;

    let mention_tuples: Vec<(String, String)> = mentions
        .into_iter()
        .map(|m| (m.entity_type, m.entity_id))
        .collect();
    core.note_repo
        .set_entity_mentions(note_id, &mention_tuples)
        .await
        .map_err(map_storage_err)?;

    Ok(())
}
