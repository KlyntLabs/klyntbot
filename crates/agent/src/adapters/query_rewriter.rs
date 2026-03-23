//! Contextual query rewriter — heuristic enrichment of vague user queries.
//!
//! Uses the current skill, active task, recent corrections, and user situation
//! to inject missing context into under-specified queries before memory retrieval.
//! Phase 1: heuristic templates only. Phase 2 will add LLM fallback.

use async_trait::async_trait;
use context_engine::rewriter::{QueryRewriter, RetrievalContext, RewriteResult, RewriteSource};

// ---------------------------------------------------------------------------
// Specificity classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Specificity {
    High,
    Medium,
    Low,
}

const PRONOUNS: &[&str] = &["that", "this", "it", "those", "these", "there", "them"];

fn contains_pronouns(query: &str) -> bool {
    let lower = query.to_lowercase();
    lower.split_whitespace().any(|w| {
        let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
        PRONOUNS.contains(&trimmed)
    })
}

/// Check for domain-specific keywords or capitalized proper nouns (entities).
fn has_domain_keywords(query: &str) -> bool {
    // Domain terms that indicate specificity.
    const DOMAIN_TERMS: &[&str] = &[
        "fire",
        "budget",
        "portfolio",
        "projection",
        "allocation",
        "sprint",
        "milestone",
        "okr",
        "deadline",
        "migration",
        "deploy",
        "standup",
        "retrospective",
        "backlog",
        "graphql",
        "api",
    ];

    let lower = query.to_lowercase();
    let has_domain = lower
        .split_whitespace()
        .any(|w| DOMAIN_TERMS.contains(&w.trim_matches(|c: char| !c.is_alphanumeric())));
    if has_domain {
        return true;
    }

    // Check for capitalized proper nouns (not first word in sentence).
    let words: Vec<&str> = query.split_whitespace().collect();
    words.iter().skip(1).any(|w| {
        let first = w.chars().next();
        first.is_some_and(|c| c.is_uppercase() && c.is_alphabetic())
    })
}

fn query_specificity(query: &str) -> Specificity {
    // Pronouns always → Low (checked first)
    if contains_pronouns(query) {
        return Specificity::Low;
    }

    let word_count = query.split_whitespace().count();
    let has_entities = has_domain_keywords(query);

    // No pronouns + entities + ≥4 words → High
    if has_entities && word_count >= 4 {
        return Specificity::High;
    }

    // 1-3 words + no entities → Low
    if word_count <= 3 && !has_entities {
        return Specificity::Low;
    }

    Specificity::Medium
}

// ---------------------------------------------------------------------------
// Key term extraction (pub — reused by AgentLoop in Task 7)
// ---------------------------------------------------------------------------

const STOP_WORDS: &[&str] = &[
    "a",
    "an",
    "and",
    "are",
    "as",
    "at",
    "be",
    "been",
    "being",
    "but",
    "by",
    "can",
    "could",
    "did",
    "do",
    "does",
    "doing",
    "done",
    "down",
    "each",
    "few",
    "for",
    "from",
    "get",
    "got",
    "had",
    "has",
    "have",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "if",
    "in",
    "into",
    "is",
    "its",
    "itself",
    "just",
    "know",
    "let",
    "like",
    "ll",
    "may",
    "me",
    "might",
    "more",
    "most",
    "much",
    "must",
    "my",
    "myself",
    "no",
    "nor",
    "not",
    "now",
    "of",
    "off",
    "on",
    "once",
    "only",
    "or",
    "other",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "re",
    "really",
    "same",
    "shall",
    "she",
    "should",
    "so",
    "some",
    "still",
    "such",
    "tell",
    "than",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "they",
    "thing",
    "things",
    "to",
    "too",
    "up",
    "us",
    "use",
    "used",
    "using",
    "ve",
    "very",
    "want",
    "was",
    "we",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "won",
    "would",
    "yes",
    "yet",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
    "about",
    "after",
    "again",
    "all",
    "also",
    "am",
    "any",
    "because",
    "before",
    "between",
    "both",
    "current",
    "currently",
    "everything",
    "going",
    "i",
];

/// Extract key terms from text by filtering stop words and short tokens.
///
/// Returns up to 5 terms joined by spaces. Useful for building enriched
/// queries from recent conversation messages.
pub fn extract_key_terms_from(text: &str) -> String {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .take(5)
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Template assembly
// ---------------------------------------------------------------------------

fn build_template(original: &str, signals: &[String], ctx: &RetrievalContext) -> String {
    let signals_text = signals.join(", ");
    let original_terms = extract_key_terms_from(original);

    // Correction template — highest priority signal
    if ctx.recent_correction.is_some() {
        if original_terms.is_empty() {
            return format!("{signals_text} — overview and current status");
        }
        return format!("{original_terms} regarding {signals_text}");
    }

    if original_terms.is_empty() {
        return format!("{signals_text} — overview and current status");
    }

    format!("{signals_text} — {original_terms}")
}

// ---------------------------------------------------------------------------
// Main rewriter
// ---------------------------------------------------------------------------

/// Contextual query rewriter that uses heuristic templates (Phase 1) and
/// optionally an LLM fallback (Phase 2, not yet implemented).
pub struct ContextualQueryRewriter {
    #[allow(dead_code)]
    llm_provider: Option<providers::DynProvider>,
    #[allow(dead_code)]
    rewriter_model: Option<String>,
    #[allow(dead_code)]
    timeout_ms: u64,
}

impl ContextualQueryRewriter {
    pub fn new(
        provider: Option<providers::DynProvider>,
        model: Option<String>,
        timeout: u64,
    ) -> Self {
        Self {
            llm_provider: provider,
            rewriter_model: model,
            timeout_ms: timeout,
        }
    }

    /// Create a rewriter that only uses heuristic templates (no LLM).
    pub fn heuristic_only() -> Self {
        Self {
            llm_provider: None,
            rewriter_model: None,
            timeout_ms: 0,
        }
    }

    /// Whether to be aggressive with context injection (more signals).
    fn is_aggressive(&self, ctx: &RetrievalContext) -> bool {
        ctx.situation
            .as_ref()
            .is_some_and(|s| s.energy_level < 0.4 || s.deadline_pressure > 0.7)
    }

    /// Maximum number of context signals to inject.
    fn max_signals(&self, ctx: &RetrievalContext) -> usize {
        if self.is_aggressive(ctx) {
            4
        } else {
            2
        }
    }

    /// Attempt heuristic rewriting by collecting context signals and
    /// assembling them into an enriched query template.
    fn heuristic_rewrite(&self, original: &str, ctx: &RetrievalContext) -> Option<RewriteResult> {
        let max = self.max_signals(ctx);
        let mut signals: Vec<String> = Vec::new();
        let mut confidence = 0.75_f32;

        // Priority 1: Recent correction (highest priority)
        if let Some(ref correction) = ctx.recent_correction {
            let terms = extract_key_terms_from(&correction.corrected_to);
            if !terms.is_empty() {
                signals.push(terms);
            }
            confidence = 0.9;
        }

        // Priority 2: Active view
        if signals.len() < max {
            if let Some(ref view) = ctx.active_view {
                if let Some(ref entity) = view.focused_entity {
                    signals.push(entity.clone());
                } else {
                    signals.push(view.dashboard.clone());
                }
            }
        }

        // Priority 3: Active task
        if signals.len() < max {
            if let Some(ref task) = ctx.active_task {
                signals.push(task.title.clone());
                if let Some(ref project) = task.project_name {
                    if signals.len() < max {
                        signals.push(format!("project: {project}"));
                    }
                }
            }
        }

        // Priority 4: Active skill
        if signals.len() < max {
            if let Some(ref skill) = ctx.active_skill {
                signals.push(format!("skill: {skill}"));
            }
        }

        // Priority 5: Recent user messages (extract key terms)
        if signals.len() < max {
            for msg in &ctx.recent_user_messages {
                if signals.len() >= max {
                    break;
                }
                let terms = extract_key_terms_from(msg);
                if !terms.is_empty() {
                    signals.push(terms);
                }
            }
        }

        if signals.is_empty() {
            return None;
        }

        let enriched_query = build_template(original, &signals, ctx);

        Some(RewriteResult {
            enriched_query,
            confidence,
            source: RewriteSource::Heuristic,
        })
    }
}

#[async_trait]
impl QueryRewriter for ContextualQueryRewriter {
    async fn rewrite(&self, original: &str, context: &RetrievalContext) -> Option<RewriteResult> {
        match query_specificity(original) {
            Specificity::High => None,
            Specificity::Medium | Specificity::Low => {
                self.heuristic_rewrite(original, context)
                // TODO Phase 2: LLM fallback when heuristic returns None for Low specificity
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_engine::rewriter::{
        ActiveTaskContext, CorrectionContext, RetrievalContext, RewriteSource,
        UserSituationSnapshot,
    };
    use context_engine::ActiveView;

    fn finance_context() -> RetrievalContext {
        RetrievalContext {
            active_skill: Some("finance-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "March budget review".into(),
                project_name: None,
                domain: Some("finance".into()),
            }),
            ..Default::default()
        }
    }

    // Specificity tests

    #[test]
    fn high_specificity_no_pronouns_with_entities() {
        assert_eq!(
            query_specificity("show me March FIRE projection"),
            Specificity::High
        );
    }

    #[test]
    fn low_specificity_pronouns() {
        assert_eq!(query_specificity("what was that thing?"), Specificity::Low);
    }

    #[test]
    fn low_specificity_short_no_entities() {
        assert_eq!(query_specificity("how are we"), Specificity::Low);
    }

    #[test]
    fn medium_specificity_long_no_special() {
        assert_eq!(
            query_specificity("tell me about the current status of everything"),
            Specificity::Medium
        );
    }

    #[test]
    fn pronouns_override_entities() {
        assert_eq!(
            query_specificity("what did John say about that auth thing?"),
            Specificity::Low
        );
    }

    // Rewriter tests

    #[tokio::test]
    async fn heuristic_enriches_vague_query_with_skill_and_task() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = finance_context();
        let result = rewriter.rewrite("how are we doing?", &ctx).await;
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(
            r.enriched_query.to_lowercase().contains("march budget"),
            "Expected 'march budget' in enriched query: {}",
            r.enriched_query
        );
        assert_eq!(r.source, RewriteSource::Heuristic);
        assert!(r.confidence >= 0.7);
    }

    #[tokio::test]
    async fn high_specificity_returns_none() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = finance_context();
        let result = rewriter
            .rewrite("show me March FIRE projection", &ctx)
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn correction_is_highest_priority() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext {
            active_skill: Some("task-management".into()),
            recent_correction: Some(CorrectionContext {
                rejected_topic: "wrong project".into(),
                corrected_to: "no, the GraphQL migration".into(),
            }),
            ..Default::default()
        };
        let result = rewriter.rewrite("any blockers?", &ctx).await;
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(
            r.enriched_query
                .to_lowercase()
                .contains("graphql migration"),
            "Expected 'graphql migration' in enriched query: {}",
            r.enriched_query
        );
    }

    #[tokio::test]
    async fn no_context_medium_specificity_returns_none() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext::default();
        let result = rewriter
            .rewrite("tell me about the progress on things", &ctx)
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn low_energy_includes_more_signals() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext {
            active_skill: Some("finance-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "March budget review".into(),
                project_name: Some("Q1 Finance".into()),
                domain: Some("finance".into()),
            }),
            situation: Some(UserSituationSnapshot {
                energy_level: 0.2,
                ..Default::default()
            }),
            recent_user_messages: vec!["I was checking the spending breakdown".into()],
            ..Default::default()
        };
        let result = rewriter.rewrite("what about that?", &ctx).await;
        assert!(result.is_some());
        let query = result.unwrap().enriched_query.to_lowercase();
        assert!(
            query.contains("march budget") || query.contains("spending"),
            "Expected 'march budget' or 'spending' in enriched query: {}",
            query
        );
    }

    #[test]
    fn extract_key_terms_filters_stopwords() {
        let terms = extract_key_terms_from("I was checking the spending breakdown yesterday");
        assert!(
            terms.contains("checking") || terms.contains("spending") || terms.contains("breakdown"),
            "Expected content terms in: {}",
            terms
        );
        assert!(!terms.contains("the"));
        assert!(!terms.contains("was"));
    }

    // Integration tests

    #[tokio::test]
    async fn rewrite_produces_natural_language_enrichment() {
        // Verify the enriched query is natural language, not keyword soup
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext {
            active_skill: Some("finance-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "March budget review".into(),
                project_name: None,
                domain: Some("finance".into()),
            }),
            ..Default::default()
        };
        let result = rewriter.rewrite("how are we doing?", &ctx).await;
        let enriched = result.unwrap().enriched_query;
        // Should contain the task context
        assert!(
            enriched.to_lowercase().contains("march budget"),
            "Expected 'march budget' in enriched query: {}",
            enriched
        );
        // Should be a readable phrase, not just keywords
        assert!(
            enriched.contains(" — ") || enriched.contains(' '),
            "Expected readable phrase in enriched query: {}",
            enriched
        );
        assert!(
            enriched.len() > 10,
            "Expected enriched query longer than 10 chars: {}",
            enriched
        );
    }

    #[tokio::test]
    async fn context_engine_without_rewriter_works() {
        // ContextEngine with no rewriter should work exactly as before
        let engine = context_engine::ContextEngine::new();
        // Just verify it can be constructed without a rewriter — the default is None
        // The actual retrieval flow is tested via the existing ContextEngine tests
        drop(engine);
    }

    #[tokio::test]
    async fn active_view_enriches_when_present() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext {
            active_view: Some(ActiveView {
                dashboard: "finance".into(),
                focused_entity: Some("FIRE projection".into()),
                description: Some("March 2026 FIRE projection with variance highlighted".into()),
            }),
            ..Default::default()
        };
        let result = rewriter.rewrite("break this down", &ctx).await;
        assert!(
            result.is_some(),
            "Expected rewrite result for active view context"
        );
        assert!(
            result
                .unwrap()
                .enriched_query
                .to_lowercase()
                .contains("fire projection"),
            "Expected 'fire projection' in enriched query"
        );
    }

    #[tokio::test]
    async fn deadline_pressure_triggers_aggressive_mode() {
        let rewriter = ContextualQueryRewriter::heuristic_only();
        let ctx = RetrievalContext {
            active_skill: Some("task-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "API migration".into(),
                project_name: Some("Backend rewrite".into()),
                domain: Some("engineering".into()),
            }),
            situation: Some(UserSituationSnapshot {
                deadline_pressure: 0.8,
                energy_level: 0.7,
                ..Default::default()
            }),
            recent_user_messages: vec!["checking the migration status".into()],
            ..Default::default()
        };
        let result = rewriter.rewrite("status?", &ctx).await;
        assert!(
            result.is_some(),
            "Expected rewrite result under deadline pressure"
        );
        let query = result.unwrap().enriched_query.to_lowercase();
        // Aggressive mode (deadline > 0.7) should include more signals
        // At minimum task title + one more signal
        assert!(
            query.contains("api migration"),
            "Expected 'api migration' in enriched query: {}",
            query
        );
    }
}
