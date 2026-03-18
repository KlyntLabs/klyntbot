//! Assemble a knowledge context block for Insight Review prompts.
//!
//! Legacy context builder — used as fallback when `InsightService` is unavailable.
//! The primary path now uses `feature_insights::PromptBuilder`.

use feature_notes::models::NoteRow;

/// Build a context block from a note and its related notes.
///
/// Returns `feature_insights::InsightContext` for compatibility with both the
/// old inline pipeline and the new `InsightService` pipeline.
pub fn assemble_context(
    note: &NoteRow,
    related_notes: &[NoteRow],
    memory_entries: Option<&[String]>,
) -> feature_insights::InsightContext {
    let mut parts = Vec::new();

    // Current note
    parts.push(format!("## Current Note: {}\n\n{}", note.title, note.body));

    // Related notes (truncate body to 2000 bytes, UTF-8 safe)
    for related in related_notes {
        let body_preview = common::truncate_at_boundary(&related.body, 2000);
        parts.push(format!(
            "## Related Note: {}\n\n{}",
            related.title, body_preview
        ));
    }

    // Cognitive memory (if available)
    if let Some(entries) = memory_entries {
        if !entries.is_empty() {
            parts.push("## Relevant Memory".to_string());
            for entry in entries {
                parts.push(format!("- {entry}"));
            }
        }
    }

    feature_insights::InsightContext {
        text: parts.join("\n\n"),
        note_title: note.title.clone(),
        related_count: related_notes.len(),
    }
}

/// Extract domain hints from a note's tags for persona selection.
/// Optionally enriches with entity types from the knowledge graph.
pub async fn extract_note_domains(
    tags: &[String],
    entity_repo: Option<&cognitive::repos::EntityRepo>,
) -> Vec<String> {
    let mut domains: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    // Enrich: look up each tag in the entity graph and add entity_type as a domain
    if let Some(repo) = entity_repo {
        for tag in tags {
            if let Ok(entities) = repo.find_by_name(tag).await {
                for entity in &entities {
                    let et = entity.entity_type.to_lowercase();
                    if !domains.contains(&et) {
                        domains.push(et);
                    }
                }
            }
        }
    }

    domains
}
