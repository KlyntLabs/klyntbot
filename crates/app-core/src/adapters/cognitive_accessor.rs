//! Concrete CognitiveAccessor — wraps cognitive repos for insight context injection.
//!
//! Medium tier (Phase 2): search_facts, recent_memories, domain_rules.
//! Deep dive (Phase 4): user_model_summary, entity_neighborhood, fact_history.

use async_trait::async_trait;
use feature_insights::CognitiveAccessor;

/// Wraps cognitive repos to provide insight context data.
pub struct CognitiveAccessorImpl {
    fact_repo: cognitive::SemanticFactRepo,
    memory_repo: cognitive::EpisodicMemoryRepo,
    rule_repo: cognitive::ProceduralRuleRepo,
}

impl CognitiveAccessorImpl {
    pub fn new(
        fact_repo: cognitive::SemanticFactRepo,
        memory_repo: cognitive::EpisodicMemoryRepo,
        rule_repo: cognitive::ProceduralRuleRepo,
    ) -> Self {
        Self {
            fact_repo,
            memory_repo,
            rule_repo,
        }
    }
}

#[async_trait]
impl CognitiveAccessor for CognitiveAccessorImpl {
    async fn search_facts(&self, query: &str, domain: Option<&str>, limit: usize) -> Vec<String> {
        // SemanticFact fields: subject, predicate, object
        self.fact_repo
            .search_fts(query, domain, limit)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|f| format!("{} {} {}", f.subject, f.predicate, f.object))
            .collect()
    }

    async fn recent_memories(&self, _note_id: &str, limit: usize) -> Vec<String> {
        // EpisodicMemory fields: content, summary (Option<String>)
        // list_recent takes i64
        self.memory_repo
            .list_recent(limit as i64)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.summary.unwrap_or(m.content))
            .collect()
    }

    async fn domain_rules(&self, domain: &str) -> Vec<String> {
        // ProceduralRule fields: rule_text, confidence
        self.rule_repo
            .list_active(domain)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| format!("{} (confidence: {:.0}%)", r.rule_text, r.confidence * 100.0))
            .collect()
    }

    // Deep dive methods — Phase 4
    async fn user_model_summary(&self, _domain: &str) -> Option<String> {
        None
    }

    async fn entity_neighborhood(&self, _note_id: &str, _depth: u8) -> Vec<String> {
        Vec::new()
    }

    async fn fact_history(&self, _subject: &str) -> Vec<String> {
        Vec::new()
    }
}
