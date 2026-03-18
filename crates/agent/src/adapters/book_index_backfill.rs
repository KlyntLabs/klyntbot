use std::sync::Arc;

use cognitive::repos::parse_markdown_to_tree;
use context_engine::book_index::types::SourceType;
use context_engine::book_index::{BookIndex, BookTreeRepo};
use tracing::info;

/// Backfill tree nodes for all existing notes that don't have trees yet.
/// Runs once at startup, non-blocking.
pub async fn backfill_existing_notes(
    note_repo: &feature_notes::repo::NoteRepo,
    tree_repo: &dyn BookTreeRepo,
    book_index: &BookIndex,
    entity_extractor: Option<
        &Arc<crate::adapters::book_index_entity_extractor::BookIndexEntityExtractor>,
    >,
) -> common::Result<u32> {
    let notes = note_repo
        .list_notes(None)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    let mut indexed = 0u32;
    for note in &notes {
        if note.body.is_empty() {
            continue;
        }

        // Delete-then-rebuild is idempotent — safe to run on already-indexed notes
        tree_repo
            .delete_by_source(&SourceType::Note, &note.id)
            .await?;

        let nodes = parse_markdown_to_tree(&note.id, &note.body);
        if nodes.is_empty() {
            continue;
        }

        tree_repo.insert_nodes(&nodes).await?;
        indexed += 1;

        if let Some(extractor) = entity_extractor {
            crate::adapters::book_index_entity_extractor::spawn_extract_and_link(
                extractor,
                nodes.clone(),
            );
        }
    }

    if indexed > 0 {
        book_index.refresh_has_content().await?;
        info!("BookIndex: backfilled {indexed} notes into tree index");
    }

    Ok(indexed)
}
