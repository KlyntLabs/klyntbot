use std::collections::HashMap;
use std::sync::Arc;

use crate::types::*;

const GENERAL_SKILL_NAME: &str = "general";
const SKILL_ACTIVATION_THRESHOLD: f64 = 0.4;
const MAX_ACTIVATED_SKILLS: usize = 3;

pub struct SkillRouter {
    /// Pre-tokenized description words per skill (for keyword scoring).
    description_tokens: HashMap<String, Vec<String>>,
}

impl SkillRouter {
    pub fn new(catalog: &SkillCatalog) -> Self {
        let mut description_tokens = HashMap::new();
        for (name, pkg) in &catalog.skills {
            let tokens: Vec<String> = tokenize(&pkg.description);
            description_tokens.insert(name.clone(), tokens);
        }
        Self { description_tokens }
    }

    /// Keyword scores for all skills against a user message.
    pub fn keyword_scores(&self, message: &str, catalog: &SkillCatalog) -> HashMap<String, f64> {
        let msg_tokens: Vec<String> = tokenize(message);
        let mut result = HashMap::new();

        for (name, desc_tokens) in &self.description_tokens {
            if !catalog.skills.contains_key(name) {
                continue;
            }
            let mut hits = 0usize;
            for token in desc_tokens {
                if msg_tokens.contains(token) {
                    hits += 1;
                }
            }
            if hits > 0 {
                let normalizer = (desc_tokens.len() as f64 / 3.0).max(1.0);
                let score = (hits as f64 / normalizer).min(1.0);
                result.insert(name.clone(), score);
            }
        }
        result
    }

    /// Select the best orchestrator for a message. Keyword-only (no embeddings).
    /// For full blended scoring, use `select_orchestrator_blended`.
    pub fn select_orchestrator<'a>(
        &self,
        message: &str,
        catalog: &'a SkillCatalog,
    ) -> &'a Arc<SkillPackage> {
        self.select_orchestrator_blended(message, &[], catalog)
    }

    /// Select orchestrator with blended keyword + semantic scoring.
    pub fn select_orchestrator_blended<'a>(
        &self,
        message: &str,
        query_embedding: &[f32],
        catalog: &'a SkillCatalog,
    ) -> &'a Arc<SkillPackage> {
        let kw_scores = self.keyword_scores(message, catalog);
        let mut best: Option<(&str, f64)> = None;

        for pkg in catalog.orchestrators() {
            let kw_score = kw_scores.get(pkg.name.as_str()).copied().unwrap_or(0.0);
            let sem_score = if !query_embedding.is_empty() {
                catalog
                    .embeddings
                    .get(&pkg.name)
                    .map(|emb| cosine_similarity(query_embedding, emb))
                    .unwrap_or(0.0)
            } else {
                0.0
            };

            // Candidacy gate: keyword hit OR semantic >= 0.5
            if kw_score == 0.0 && sem_score < 0.5 {
                continue;
            }

            let blended = kw_score * 0.7 + sem_score * 0.3;
            if best.as_ref().map_or(true, |(_, s)| blended > *s) {
                best = Some((pkg.name.as_str(), blended));
            }
        }

        if let Some((name, _)) = best {
            catalog.get(name).unwrap()
        } else {
            catalog
                .get(GENERAL_SKILL_NAME)
                .expect("General orchestrator must exist")
        }
    }

    /// Activate non-orchestrator skills relevant to the message.
    pub fn activate_skills<'a>(
        &self,
        message: &str,
        query_embedding: &[f32],
        catalog: &'a SkillCatalog,
    ) -> Vec<&'a Arc<SkillPackage>> {
        let kw_scores = self.keyword_scores(message, catalog);
        let mut scored: Vec<(&Arc<SkillPackage>, f64)> = Vec::new();

        for pkg in catalog.regular_skills() {
            let kw_score = kw_scores.get(pkg.name.as_str()).copied().unwrap_or(0.0);
            let sem_score = if !query_embedding.is_empty() {
                catalog
                    .embeddings
                    .get(&pkg.name)
                    .map(|emb| cosine_similarity(query_embedding, emb))
                    .unwrap_or(0.0)
            } else {
                0.0
            };

            let blended = kw_score * 0.7 + sem_score * 0.3;
            if blended >= SKILL_ACTIVATION_THRESHOLD {
                scored.push((pkg, blended));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(MAX_ACTIVATED_SKILLS);
        scored.into_iter().map(|(pkg, _)| pkg).collect()
    }
}

/// Common English stop words that cause false-positive keyword matches.
const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "can", "has", "her", "was", "one",
    "our", "out", "use", "when", "will", "how", "any", "its", "say", "she", "which", "their",
    "them", "then", "than", "been", "have", "from", "with", "they", "this", "that", "what", "does",
    "doesn", "don", "didn", "isn", "aren", "won", "wouldn", "couldn", "shouldn",
];

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('-', " ")
        .split_whitespace()
        .filter(|w| w.len() > 2) // Skip tiny words (a, an, to, etc.)
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty() && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        (dot / (norm_a * norm_b)) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::SkillSource;

    fn make_test_catalog() -> SkillCatalog {
        let skills = vec![
            ("task-management".to_string(), "---\nname: task-management\ndescription: Create, organize, and track tasks, projects, and areas using OKR+PARA. Use when the user mentions todos, tasks, projects, areas, objectives, planning, reviews, or goal tracking.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nTask instructions.".to_string()),
            ("finance-management".to_string(), "---\nname: finance-management\ndescription: Track expenses, budgets, and financial goals. Use when the user mentions budget, spending, expenses, or financial tracking.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nFinance instructions.".to_string()),
            ("general".to_string(), "---\nname: general\ndescription: General-purpose assistant and orchestrator for greetings, conversation, and unmatched requests.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nGeneral instructions.".to_string()),
            ("search".to_string(), "---\nname: search\ndescription: Web search and information retrieval for factual questions.\n---\nSearch instructions.".to_string()),
        ];
        let source = SkillSource::BuiltIn(skills);
        SkillCatalog::discover_sync(&[source]).unwrap()
    }

    #[test]
    fn test_keyword_scores_task_message() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let scores = router.keyword_scores("create a task for reviewing the budget", &catalog);
        assert!(scores.contains_key("task-management"));
        assert!(*scores.get("task-management").unwrap() > 0.0);
    }

    #[test]
    fn test_keyword_scores_no_match() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let scores = router.keyword_scores("hello, how are you today?", &catalog);
        let task_score = scores.get("task-management").copied().unwrap_or(0.0);
        let finance_score = scores.get("finance-management").copied().unwrap_or(0.0);
        assert!(task_score == 0.0 || task_score < 0.2);
        assert!(finance_score == 0.0 || finance_score < 0.2);
    }

    #[test]
    fn test_select_orchestrator_falls_back_to_general() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let selected = router.select_orchestrator("hello there!", &catalog);
        assert_eq!(selected.name, "general");
    }

    #[test]
    fn test_select_orchestrator_only_returns_orchestrators() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let selected = router.select_orchestrator("search the web for me", &catalog);
        assert_eq!(selected.skill_type, SkillType::Orchestrator);
    }
}
