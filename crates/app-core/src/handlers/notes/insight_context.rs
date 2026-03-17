//! Assemble a knowledge context block for Insight Review prompts.
//!
//! Gathers: current note, related notes (backlinks), and optionally
//! cognitive memory entries. Formats into a structured text block
//! that LLM prompts can consume.

use feature_notes::models::NoteRow;

/// Assembled context ready for prompt injection.
#[allow(dead_code)]
pub struct InsightContext {
    pub text: String,
    pub note_title: String,
    pub related_count: usize,
}

/// Build a context block from a note and its related notes.
pub fn assemble_context(
    note: &NoteRow,
    related_notes: &[NoteRow],
    memory_entries: Option<&[String]>,
) -> InsightContext {
    let mut parts = Vec::new();

    // Current note
    parts.push(format!("## Current Note: {}\n\n{}", note.title, note.body));

    // Related notes (truncate body to 2000 chars each to stay within token limits)
    for related in related_notes {
        let body_preview = if related.body.len() > 2000 {
            format!("{}...", &related.body[..2000])
        } else {
            related.body.clone()
        };
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

    InsightContext {
        text: parts.join("\n\n"),
        note_title: note.title.clone(),
        related_count: related_notes.len(),
    }
}
