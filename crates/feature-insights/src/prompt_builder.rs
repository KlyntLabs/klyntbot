//! Prompt context builder for insight generation.
//!
//! Assembles the full context string from:
//! 1. Target note (title + body)
//! 2. Related notes from scope resolution
//! 3. Cognitive data (facts, episodic memories, procedural rules)
//! 4. Parent insight summary (from smart merge, if applicable)

use std::sync::Arc;

use feature_notes::models::NoteRow;

use crate::traits::CognitiveAccessor;
use crate::types::{InsightContent, InsightReviewRow, ScopeConfig};

/// Assembled context ready for prompt injection.
pub struct InsightContext {
    pub text: String,
    pub note_title: String,
    pub related_count: usize,
}

/// Builds the full context for insight prompt injection.
pub struct PromptBuilder {
    cognitive: Arc<dyn CognitiveAccessor>,
}

impl PromptBuilder {
    pub fn new(cognitive: Arc<dyn CognitiveAccessor>) -> Self {
        Self { cognitive }
    }

    /// Assemble the full context for insight generation.
    ///
    /// - `note`: the target note
    /// - `related_notes`: notes resolved by the ScopeResolver
    /// - `scope_config`: controls whether cognitive data is included
    /// - `domains`: domain hints extracted from note tags
    /// - `parent`: optional parent insight from smart merge
    pub async fn build_context(
        &self,
        note: &NoteRow,
        related_notes: &[NoteRow],
        scope_config: &ScopeConfig,
        domains: &[String],
        parent: Option<&InsightReviewRow>,
    ) -> InsightContext {
        let mut sections: Vec<String> = Vec::new();

        // Section 1: Target note
        sections.push(format!("## Current Note: {}\n\n{}", note.title, note.body));

        // Section 2: Related notes
        for related in related_notes {
            let body_preview = truncate_body(&related.body, 2000);
            sections.push(format!(
                "## Related Note: {}\n\n{}",
                related.title, body_preview
            ));
        }

        // Section 3: Cognitive context (medium tier, when enabled)
        // Run all three queries concurrently since they hit independent tables.
        if scope_config.include_cognitive {
            let domain = domains.first().map(|s| s.as_str());

            let (facts, memories, rules, atom_subjects) = tokio::join!(
                self.cognitive.search_facts(&note.title, domain, 10),
                self.cognitive.recent_memories(&note.id, 5),
                async {
                    match domain {
                        Some(d) => self.cognitive.domain_rules(d).await,
                        None => Vec::new(),
                    }
                },
                self.cognitive.search_atoms(&note.id),
            );

            if !facts.is_empty() {
                sections.push(format!("## Relevant Knowledge\n\n{}", bullet_list(&facts)));
            }

            if !memories.is_empty() {
                sections.push(format!(
                    "## Recent Learning Sessions\n\n{}",
                    bullet_list(&memories)
                ));
            }

            if !rules.is_empty() {
                sections.push(format!("## Domain Insights\n\n{}", bullet_list(&rules)));
            }

            if !atom_subjects.is_empty() {
                sections.push(format!(
                    "## Already Learned\nThe user has accepted these concepts as known: {}.\n\
                     Consider these as established knowledge — don't re-explain them in the synthesis.\n\
                     Focus gap analysis on what's NOT yet covered.",
                    atom_subjects.join(", ")
                ));
            }
        }

        // Section 3b: Deep dive context (user model, entity graph, fact history)
        if scope_config.deep_dive {
            let note_title_for_subject = note.title.clone();
            let (user_model, neighborhood, history) = tokio::join!(
                self.cognitive.user_model_summary(""),
                self.cognitive.entity_neighborhood(&note.id, 2),
                self.cognitive.fact_history(&note_title_for_subject),
            );

            let mut deep_parts = Vec::new();
            if let Some(model) = user_model {
                deep_parts.push(format!("### User Model\n{model}"));
            }
            if !neighborhood.is_empty() {
                deep_parts.push(format!(
                    "### Entity Connections\n{}",
                    bullet_list(&neighborhood)
                ));
            }
            if !history.is_empty() {
                deep_parts.push(format!(
                    "### Knowledge Evolution\n{}",
                    bullet_list(&history)
                ));
            }
            if !deep_parts.is_empty() {
                sections.push(format!(
                    "## Deep Dive Context\n\n{}",
                    deep_parts.join("\n\n")
                ));
            }
        }

        // Section 4: Parent insight context (from smart merge)
        if let Some(parent_row) = parent {
            if let Ok(parent_content) = serde_json::from_str::<InsightContent>(&parent_row.content)
            {
                let mut parent_sections = Vec::new();
                if let Some(ref syn) = parent_content.synthesis {
                    parent_sections.push(format!("Prior synthesis:\n{}", truncate_body(syn, 1000)));
                }
                if let Some(ref gaps) = parent_content.gap_analysis {
                    parent_sections.push(format!(
                        "Prior gaps identified:\n{}",
                        truncate_body(gaps, 500)
                    ));
                }
                if !parent_sections.is_empty() {
                    sections.push(format!(
                        "## Prior Analysis (from related insight, generated {})\n\n{}\n\n\
                        Focus on what's NEW, DIFFERENT, or CONTRADICTORY compared to this prior analysis. \
                        If the current note closes any prior gaps, note that explicitly.",
                        parent_row.generated_at,
                        parent_sections.join("\n\n")
                    ));
                }
            }
        }

        let related_count = related_notes.len();
        InsightContext {
            text: sections.join("\n\n"),
            note_title: note.title.clone(),
            related_count,
        }
    }
}

/// Format items as a markdown bullet list.
fn bullet_list(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!("- {s}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Truncate a string to approximately `max_bytes`, safe for UTF-8.
fn truncate_body(body: &str, max_bytes: usize) -> &str {
    common::truncate_at_boundary(body, max_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::NoopCognitiveAccessor;

    fn test_note(id: &str, title: &str, body: &str) -> NoteRow {
        NoteRow {
            id: id.to_string(),
            notebook_id: None,
            title: title.to_string(),
            body: body.to_string(),
            body_html: None,
            pinned: 0,
            archived: 0,
            icon: None,
            color: None,
            embedding_updated_at: None,
            split_content: None,
            split_mode: None,
            perspective_config: None,
            last_visited_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[tokio::test]
    async fn test_build_context_basic() {
        let cognitive = Arc::new(NoopCognitiveAccessor);
        let builder = PromptBuilder::new(cognitive);

        let note = test_note("note-1", "Test Note", "Some content");
        let related = vec![test_note("note-2", "Related", "Related content")];
        let scope = ScopeConfig::default();

        let ctx = builder
            .build_context(&note, &related, &scope, &[], None)
            .await;

        assert!(ctx.text.contains("## Current Note: Test Note"));
        assert!(ctx.text.contains("## Related Note: Related"));
        assert_eq!(ctx.related_count, 1);
        assert_eq!(ctx.note_title, "Test Note");
    }

    #[tokio::test]
    async fn test_build_context_with_parent() {
        let cognitive = Arc::new(NoopCognitiveAccessor);
        let builder = PromptBuilder::new(cognitive);

        let note = test_note("note-1", "Test", "Content");

        let parent_content = InsightContent {
            synthesis: Some("Previous synthesis text".to_string()),
            gap_analysis: Some("- Missing topic X\n- Shallow coverage of Y".to_string()),
            ..Default::default()
        };
        let parent_row = InsightReviewRow {
            id: "parent-id".to_string(),
            note_id: "other-note".to_string(),
            version: 1,
            generated_at: "2026-03-17T00:00:00Z".to_string(),
            content: serde_json::to_string(&parent_content).unwrap(),
            input_hash: "hash".to_string(),
            scope_config: "{}".to_string(),
            persona_ids: "[]".to_string(),
            parent_insight_id: None,
            token_cost_usd: None,
            superseded_at: None,
        };

        let scope = ScopeConfig {
            include_cognitive: false,
            ..Default::default()
        };

        let ctx = builder
            .build_context(&note, &[], &scope, &[], Some(&parent_row))
            .await;

        assert!(ctx.text.contains("## Prior Analysis"));
        assert!(ctx.text.contains("Previous synthesis text"));
        assert!(ctx.text.contains("Missing topic X"));
        assert!(ctx.text.contains("NEW, DIFFERENT, or CONTRADICTORY"));
    }

    #[tokio::test]
    async fn test_build_context_no_cognitive_when_disabled() {
        let cognitive = Arc::new(NoopCognitiveAccessor);
        let builder = PromptBuilder::new(cognitive);

        let note = test_note("note-1", "Test", "Content");
        let scope = ScopeConfig {
            include_cognitive: false,
            ..Default::default()
        };

        let ctx = builder.build_context(&note, &[], &scope, &[], None).await;

        // With noop cognitive + disabled flag, only the note section should appear
        assert!(ctx.text.contains("## Current Note"));
        assert!(!ctx.text.contains("## Relevant Knowledge"));
        assert!(!ctx.text.contains("## Recent Learning"));
    }

    #[test]
    fn test_truncate_body() {
        assert_eq!(truncate_body("hello world", 20), "hello world");
        // truncate_at_boundary truncates at char boundary, not word boundary
        assert_eq!(truncate_body("hello world foo bar", 15), "hello world foo");
        assert_eq!(truncate_body("hello world foo bar", 5), "hello");
    }
}
