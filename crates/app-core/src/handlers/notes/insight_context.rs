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
pub fn extract_note_domains(tags: &[String]) -> Vec<String> {
    tags.iter().map(|t| t.to_lowercase()).collect()
}
