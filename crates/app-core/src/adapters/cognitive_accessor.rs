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
    entity_repo: cognitive::repos::EntityRepo,
    temporal_svc: cognitive::TemporalService,
    atom_repo: cognitive::KnowledgeAtomRepo,
}

impl CognitiveAccessorImpl {
    pub fn new(
        fact_repo: cognitive::SemanticFactRepo,
        memory_repo: cognitive::EpisodicMemoryRepo,
        rule_repo: cognitive::ProceduralRuleRepo,
        entity_repo: cognitive::repos::EntityRepo,
        atom_repo: cognitive::KnowledgeAtomRepo,
    ) -> Self {
        let temporal_svc = cognitive::TemporalService::new(fact_repo.clone());
        Self {
            fact_repo,
            memory_repo,
            rule_repo,
            entity_repo,
            temporal_svc,
            atom_repo,
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

    async fn user_model_summary(&self, _domain: &str) -> Option<String> {
        let model = cognitive::repos::load_user_model(&self.fact_repo).await;
        let mut parts = Vec::new();

        let domains: &[(&str, &[cognitive::SemanticFact])] = &[
            ("identity", &model.identity),
            ("energy", &model.energy),
            ("work", &model.work),
            ("finance", &model.finance),
            ("learning", &model.learning),
            ("preferences", &model.preferences),
        ];

        for &(name, facts) in domains {
            if !facts.is_empty() {
                let summary: Vec<String> = facts
                    .iter()
                    .take(3)
                    .map(|f| format!("{} {}", f.predicate, f.object))
                    .collect();
                parts.push(format!("**{}:** {}", name, summary.join("; ")));
            }
        }

        if parts.is_empty() {
            return None;
        }
        Some(parts.join("\n"))
    }

    async fn entity_neighborhood(&self, note_id: &str, depth: u8) -> Vec<String> {
        match self
            .entity_repo
            .get_neighborhood(note_id, depth as u32)
            .await
        {
            Ok(Some(neighborhood)) => {
                // Build ID → name lookup from center + neighbors
                let mut names: std::collections::HashMap<&str, &str> =
                    std::collections::HashMap::new();
                names.insert(&neighborhood.center.id, &neighborhood.center.name);
                for n in &neighborhood.neighbors {
                    names.insert(&n.id, &n.name);
                }

                neighborhood
                    .relationships
                    .iter()
                    .map(|rel| {
                        let source = names
                            .get(rel.source_entity_id.as_str())
                            .copied()
                            .unwrap_or(&rel.source_entity_id);
                        let target = names
                            .get(rel.target_entity_id.as_str())
                            .copied()
                            .unwrap_or(&rel.target_entity_id);
                        format!("{source} → {} → {target}", rel.relationship_type)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    async fn fact_history(&self, subject: &str) -> Vec<String> {
        match self.temporal_svc.get_fact_history(subject, "%").await {
            Ok(versions) => versions
                .iter()
                .take(10)
                .map(|v| {
                    format!(
                        "[{}] {} {} → {}",
                        &v.fact.valid_from, subject, v.fact.predicate, v.fact.object,
                    )
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    async fn search_atoms(&self, note_id: &str) -> Vec<String> {
        match self.atom_repo.list_for_note(note_id).await {
            Ok(atoms) => atoms
                .into_iter()
                .filter(|a| a.status == "active")
                .map(|a| a.subject)
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}
