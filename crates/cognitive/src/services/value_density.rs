//! Heuristic value-density classifier for conversation turns.
//!
//! Scores each turn on four signals without any LLM call:
//! - entity_signal  (0.30) — named entities detected (capitalized words, patterns)
//! - action_signal  (0.25) — action verbs present (decided, created, changed, etc.)
//! - decision_signal (0.25) — decision markers (because, therefore, will, should, etc.)
//! - novelty_signal (0.20) — references to previously unseen terms
//!
//! Three tiers:
//! - High  (>0.7) — immediate enrichment
//! - Medium (0.4–0.7) — queued for Reforge Phase 6.5
//! - Low   (<0.4) — cheap extraction only

/// Weights for each signal component.
const W_ENTITY: f64 = 0.30;
const W_ACTION: f64 = 0.25;
const W_DECISION: f64 = 0.25;
const W_NOVELTY: f64 = 0.20;

/// Action verbs that indicate information-rich content.
const ACTION_VERBS: &[&str] = &[
    "decided",
    "created",
    "changed",
    "moved",
    "started",
    "finished",
    "cancelled",
    "approved",
    "rejected",
    "deployed",
    "fixed",
    "broke",
    "shipped",
    "migrated",
    "refactored",
    "implemented",
    "designed",
    "reviewed",
    "merged",
    "released",
    "hired",
    "fired",
    "promoted",
    "scheduled",
    "booked",
    "bought",
    "sold",
    "invested",
    "transferred",
    "configured",
    "installed",
    "updated",
];

/// Decision markers that indicate reasoning or commitments.
const DECISION_MARKERS: &[&str] = &[
    "because",
    "therefore",
    "decided",
    "will",
    "should",
    "must",
    "going to",
    "plan to",
    "chose",
    "picked",
    "settled on",
    "committed to",
    "agreed",
    "prefer",
    "instead of",
    "rather than",
    "the reason",
    "due to",
];

/// Density tier thresholds.
const HIGH_THRESHOLD: f64 = 0.7;
const MEDIUM_THRESHOLD: f64 = 0.4;

/// Value-density tier for a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityTier {
    High,
    Medium,
    Low,
}

impl DensityTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

/// Result of scoring a conversation turn.
#[derive(Debug, Clone)]
pub struct DensityScore {
    pub total: f64,
    pub entity_signal: f64,
    pub action_signal: f64,
    pub decision_signal: f64,
    pub novelty_signal: f64,
    pub tier: DensityTier,
}

/// Score a conversation turn's value-density using lightweight heuristics.
///
/// `known_entities` is an optional set of entity names already in the graph.
/// If provided, references to unknown entities boost the novelty signal.
pub fn score_turn(content: &str, known_entities: Option<&[String]>) -> DensityScore {
    let lower = content.to_lowercase();
    let words: Vec<&str> = content.split_whitespace().collect();

    if words.is_empty() {
        return DensityScore {
            total: 0.0,
            entity_signal: 0.0,
            action_signal: 0.0,
            decision_signal: 0.0,
            novelty_signal: 0.0,
            tier: DensityTier::Low,
        };
    }

    let word_count = words.len() as f64;

    // Entity signal: capitalized words that aren't sentence starters
    let entity_count =
        crate::services::graph_retrieval::extract_query_entities(content).len();
    let entity_signal = (entity_count as f64 / word_count * 4.0).min(1.0);

    // Action signal: count of action verbs
    let action_count = ACTION_VERBS.iter().filter(|v| lower.contains(**v)).count();
    let action_signal = (action_count as f64 / 3.0).min(1.0);

    // Decision signal: count of decision markers
    let decision_count = DECISION_MARKERS
        .iter()
        .filter(|m| lower.contains(**m))
        .count();
    let decision_signal = (decision_count as f64 / 2.0).min(1.0);

    // Novelty signal: references to entities not in known set
    let novelty_signal = if let Some(known) = known_entities {
        let known_lower: Vec<String> = known.iter().map(|e| e.to_lowercase()).collect();
        let novel_count = words
            .iter()
            .enumerate()
            .filter(|(i, w)| {
                *i > 0
                    && w.len() > 1
                    && w.chars().next().is_some_and(|c| c.is_uppercase())
                    && !known_lower.iter().any(|k| k == &w.to_lowercase())
            })
            .count();
        (novel_count as f64 / word_count * 5.0).min(1.0)
    } else {
        // Without known entities, use a proxy: ratio of capitalized words
        entity_signal * 0.5
    };

    let total = entity_signal * W_ENTITY
        + action_signal * W_ACTION
        + decision_signal * W_DECISION
        + novelty_signal * W_NOVELTY;

    let tier = if total >= HIGH_THRESHOLD {
        DensityTier::High
    } else if total >= MEDIUM_THRESHOLD {
        DensityTier::Medium
    } else {
        DensityTier::Low
    };

    DensityScore {
        total,
        entity_signal,
        action_signal,
        decision_signal,
        novelty_signal,
        tier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_density_question() {
        let score = score_turn("What time is it?", None);
        assert_eq!(score.tier, DensityTier::Low);
        assert!(score.total < MEDIUM_THRESHOLD);
    }

    #[test]
    fn high_density_decision() {
        let content = "I decided to migrate the Klynt project to Rust because \
            TypeScript was too slow. Started the refactoring yesterday and \
            deployed the first module to Production today.";
        let score = score_turn(content, None);
        assert!(
            score.total >= MEDIUM_THRESHOLD,
            "Decision-rich content should be at least medium, got {:.2}",
            score.total
        );
        assert!(score.action_signal > 0.0);
        assert!(score.decision_signal > 0.0);
    }

    #[test]
    fn novelty_boost_with_unknown_entities() {
        let known = vec!["Rust".to_string(), "Jayden".to_string()];
        let content = "I told Sarah about the Acme project we discussed with Bob at Google";
        let score = score_turn(content, Some(&known));
        assert!(
            score.novelty_signal > 0.0,
            "Unknown entities should boost novelty"
        );
    }

    #[test]
    fn empty_content() {
        let score = score_turn("", None);
        assert_eq!(score.tier, DensityTier::Low);
        assert_eq!(score.total, 0.0);
    }
}
