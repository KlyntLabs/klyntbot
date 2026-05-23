//! Tree-node and entity backfill routines for startup indexing.

use tracing::{info, warn};

/// Backfill tree nodes for all existing notes that may not have been indexed yet.
///
/// Iterates through all non-archived notes in batches, calling
/// `NoteTreeBuilder::handle_note_changed` for each. The builder is idempotent
/// (deletes old nodes before inserting), so re-processing already-indexed notes
/// is safe but wasteful — a cheap cost for correctness during the migration
/// window.
pub(crate) async fn backfill_tree_nodes(
    note_repo: &feature_notes::repo::NoteRepo,
    tree_builder: &crate::adapters::note_tree_builder::NoteTreeBuilder,
) -> common::Result<()> {
    let batch_size: i64 = 50;
    let mut offset: i64 = 0;
    let mut processed: usize = 0;

    loop {
        let notes = note_repo
            .list_all_notes_paginated(batch_size, offset)
            .await?;

        if notes.is_empty() {
            break;
        }

        for note in &notes {
            if note.body.is_empty() && note.body_json.is_none() {
                continue;
            }
            // Prefer body_json (Tiptap) for richer tree parsing; fall back to body (markdown).
            let content = note.body_json.as_deref().unwrap_or(&note.body);
            if let Err(e) = tree_builder.handle_note_changed(&note.id, content).await {
                warn!(
                    note_id = %note.id,
                    "Tree backfill failed for note: {e}"
                );
            }
            tokio::task::yield_now().await;
        }

        processed += notes.len();
        offset += batch_size;
        tracing::debug!("Tree backfill progress: {processed} notes");
    }

    if processed > 0 {
        info!("Note tree backfill complete: {processed} notes processed");
    }
    Ok(())
}

/// Backfill entity-tree links for all sources that have tree nodes.
/// Runs after tree node backfill to ensure tree nodes exist first.
pub(crate) async fn backfill_entity_links(
    linker: &crate::adapters::entity_tree_linker::EntityTreeLinker,
) {
    let pool = linker.pool();
    let source_rows: Vec<(String, String)> = match sqlx::query_as(
        "SELECT DISTINCT source_type, source_id FROM book_tree_nodes WHERE source_type IN ('note', 'task')",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Entity link backfill: failed to query source IDs: {e}");
            return;
        }
    };

    let mut linked = 0usize;
    for (source_type, source_id) in &source_rows {
        if let Err(e) = linker
            .link_entities_for_source(source_type, source_id)
            .await
        {
            warn!(source_type = %source_type, source_id = %source_id, "Entity link backfill failed: {e}");
        } else {
            linked += 1;
        }
        tokio::task::yield_now().await;
    }

    if linked > 0 {
        info!("Entity link backfill complete: {linked} sources processed");
    }
}

/// Backfill task trees for all existing projects.
pub(crate) async fn backfill_task_trees(
    builder: &crate::adapters::task_tree_builder::TaskTreeBuilder,
) -> common::Result<()> {
    let pool = builder.pool();
    let project_ids: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT id FROM projects")
        .fetch_all(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    let mut processed = 0usize;
    for (project_id,) in &project_ids {
        if let Err(e) = builder.handle_project_changed(project_id).await {
            warn!(project_id = %project_id, "Task tree backfill failed: {e}");
        } else {
            processed += 1;
        }
        tokio::task::yield_now().await;
    }

    if processed > 0 {
        info!("Task tree backfill complete: {processed} projects processed");
    }
    Ok(())
}
