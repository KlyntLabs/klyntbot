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
        let msg_lower = message.to_lowercase();
        let msg_tokens: Vec<String> = tokenize(message);
        let mut result = HashMap::new();

        for (name, desc_tokens) in &self.description_tokens {
            let pkg = match catalog.skills.get(name) {
                Some(p) => p,
                None => continue,
            };

            // Description keyword matching (existing logic)
            let mut hits = 0usize;
            for token in desc_tokens {
                if msg_tokens.contains(token) {
                    hits += 1;
                }
            }

            // Trigger phrase matching — exact substring match (case-insensitive)
            let trigger_hits = pkg
                .triggers()
                .iter()
                .filter(|t| msg_lower.contains(&t.to_lowercase()))
                .count();

            if hits > 0 || trigger_hits > 0 {
                let normalizer = (desc_tokens.len() as f64 / 3.0).max(1.0);
                let desc_score = (hits as f64 / normalizer).min(1.0);
                // Each trigger match adds 0.3, capped at 1.0
                let trigger_score = (trigger_hits as f64 * 0.3).min(1.0);
                let score = (desc_score + trigger_score).min(1.0);
                result.insert(name.clone(), score);
            }
        }
        result
    }

    /// Select the best orchestrator for a message. Keyword-only (no embeddings).
    /// When `champion_params` is provided, uses tuned keyword/semantic weights.
    pub fn select_orchestrator<'a>(
        &self,
        message: &str,
        catalog: &'a SkillCatalog,
        champion_params: Option<&common::TrialParams>,
    ) -> &'a Arc<SkillPackage> {
        let (kw_w, sem_w) = champion_params
            .map(|p| (p.skill_keyword_weight, p.skill_semantic_weight))
            .unwrap_or((None, None));
        self.select_orchestrator_blended(message, &[], catalog, kw_w, sem_w)
    }

    /// Select orchestrator with blended keyword + semantic scoring.
    ///
    /// `keyword_weight` and `semantic_weight` override the default 0.7 / 0.3
    /// blend weights when supplied. Pass `None` to use the defaults.
    ///
    /// When the top two candidates are within `AMBIGUITY_THRESHOLD`, picks the
    /// more specific skill (fewer trigger phrases) as a tiebreaker.
    pub fn select_orchestrator_blended<'a>(
        &self,
        message: &str,
        query_embedding: &[f32],
        catalog: &'a SkillCatalog,
        keyword_weight: Option<f64>,
        semantic_weight: Option<f64>,
    ) -> &'a Arc<SkillPackage> {
        const AMBIGUITY_THRESHOLD: f64 = 0.05;

        let kw_w = keyword_weight.unwrap_or(0.7);
        let sem_w = semantic_weight.unwrap_or(0.3);
        let kw_scores = self.keyword_scores(message, catalog);

        // Collect all candidates that pass the candidacy gate
        let mut candidates: Vec<(&str, f64)> = Vec::new();
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

            let blended = kw_score * kw_w + sem_score * sem_w;
            candidates.push((pkg.name.as_str(), blended));
        }

        // Sort descending by score
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Disambiguation: if top two are within threshold, prefer the more specific skill
        if candidates.len() >= 2 {
            let gap = candidates[0].1 - candidates[1].1;
            if gap < AMBIGUITY_THRESHOLD {
                let a_triggers = catalog
                    .get(candidates[0].0)
                    .map_or(0, |s| s.triggers().len());
                let b_triggers = catalog
                    .get(candidates[1].0)
                    .map_or(0, |s| s.triggers().len());
                // More specific = fewer triggers (more targeted)
                if b_triggers < a_triggers && b_triggers > 0 {
                    candidates.swap(0, 1);
                }
            }
        }

        if let Some((name, _)) = candidates.first() {
            catalog.get(name).unwrap()
        } else {
            catalog
                .get(GENERAL_SKILL_NAME)
                .expect("General orchestrator must exist")
        }
    }

    /// Activate non-orchestrator skills relevant to the message.
    /// `activation_threshold` overrides the default 0.4 when provided.
    pub fn activate_skills<'a>(
        &self,
        message: &str,
        query_embedding: &[f32],
        catalog: &'a SkillCatalog,
        activation_threshold: Option<f64>,
    ) -> Vec<&'a Arc<SkillPackage>> {
        let threshold = activation_threshold.unwrap_or(SKILL_ACTIVATION_THRESHOLD);
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
            if blended >= threshold {
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

// Re-use canonical implementation from common crate.
use common::helpers::cosine_similarity;

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
        let selected = router.select_orchestrator("hello there!", &catalog, None);
        assert_eq!(selected.name, "general");
    }

    #[test]
    fn test_select_orchestrator_only_returns_orchestrators() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let selected = router.select_orchestrator("search the web for me", &catalog, None);
        assert_eq!(selected.skill_type, SkillType::Orchestrator);
    }

    #[test]
    fn router_boosts_score_for_trigger_match() {
        let skills = vec![
            (
                "task-management".to_string(),
                "---\nname: task-management\ndescription: Manage tasks and todos.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    triggers:\n      - \"add task\"\n      - \"create todo\"\n---\nTask management body."
                    .to_string(),
            ),
            (
                "general".to_string(),
                "---\nname: general\ndescription: General purpose assistant.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nGeneral body."
                    .to_string(),
            ),
        ];
        let source = SkillSource::BuiltIn(skills);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        let router = SkillRouter::new(&catalog);

        let scores = router.keyword_scores("add task to my list", &catalog);
        let task_score = scores.get("task-management").copied().unwrap_or(0.0);
        assert!(task_score > 0.0, "trigger phrase should produce a score");
    }

    #[test]
    fn disambiguation_picks_more_specific_on_tie() {
        // Create two orchestrators with very similar descriptions to trigger ambiguity
        let skills = vec![
            (
                "broad-skill".to_string(),
                "---\nname: broad-skill\ndescription: Manage tasks, schedule, budget, planning and reviews.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    triggers:\n      - \"task\"\n      - \"plan\"\n      - \"budget\"\n      - \"schedule\"\n      - \"review\"\n---\nBroad body."
                    .to_string(),
            ),
            (
                "narrow-skill".to_string(),
                "---\nname: narrow-skill\ndescription: Manage tasks and planning.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    triggers:\n      - \"plan my tasks\"\n---\nNarrow body."
                    .to_string(),
            ),
            (
                "general".to_string(),
                "---\nname: general\ndescription: General purpose.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nGeneral body."
                    .to_string(),
            ),
        ];
        let source = SkillSource::BuiltIn(skills);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        let router = SkillRouter::new(&catalog);

        // "plan my tasks" triggers both skills — narrow should win due to fewer triggers
        let selected = router.select_orchestrator_blended(
            "plan my tasks for the week",
            &[],
            &catalog,
            None,
            None,
        );
        // The narrow skill has only 1 trigger (more specific) vs broad's 5
        assert_eq!(
            selected.name, "narrow-skill",
            "Disambiguation should prefer the more specific (fewer triggers) skill"
        );
    }

    #[test]
    fn test_report_triggers_route_to_task_management() {
        let builtin: Vec<(String, String)> = crate::discovery::BUILTIN_SKILLS
            .iter()
            .map(|(n, c)| (n.to_string(), c.to_string()))
            .collect();
        let source = SkillSource::BuiltIn(builtin);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        let router = SkillRouter::new(&catalog);

        let pkg = router.select_orchestrator("weekly report", &catalog, None);
        assert_eq!(
            pkg.name, "task-management",
            "weekly report should route to task-management"
        );

        let pkg = router.select_orchestrator("finance report", &catalog, None);
        assert_eq!(
            pkg.name, "finance-management",
            "finance report should route to finance-management"
        );
    }
}
