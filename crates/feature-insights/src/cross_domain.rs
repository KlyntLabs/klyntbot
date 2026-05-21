//! Cross-domain heuristic — pure function that detects connections between
//! entities from different feature domains using semantic, temporal, and
//! frequency signals.

use jiff::Timestamp;
use serde::Serialize;

/// The feature domain an entity belongs to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityDomain {
    Task,
    Note,
    Productivity,
}

/// A lightweight reference to a domain entity.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRef {
    pub domain: EntityDomain,
    pub id: String,
    pub title: String,
}

/// A single evidence layer that contributed to a cross-domain match.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// The two entities share semantically similar content (cosine >= threshold).
    SemanticOverlap { cosine: f64 },
    /// The two entities were created within a short time window of each other.
    TemporalProximity { days_apart: i64 },
    /// The two entities co-occur frequently across the user's data.
    FrequencySignal { mentions: u32, features: u32 },
}

/// A confirmed cross-domain connection ready for UI surfacing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossDomainDot {
    pub source: EntityRef,
    pub target: EntityRef,
    pub layers_matched: Vec<Layer>,
    pub confidence: f64,
    pub tooltip: String,
    pub detail_route: String,
}

/// Everything the heuristic needs to evaluate a single source entity.
pub struct HeuristicInput {
    /// The entity we are evaluating connections for.
    pub source: EntityRef,
    /// When the source entity was created.
    pub source_created: Timestamp,
    /// Vector-search results: (candidate entity, cosine score, created timestamp).
    pub vector_hits: Vec<(EntityRef, f64, Timestamp)>,
    /// Frequency co-occurrence data: (entity_id, mention_count, feature_count).
    pub frequency_data: Vec<(String, u32, u32)>,
    /// Whether the source entity has meaningful content worth connecting.
    pub source_has_content: bool,
}

/// Tunable thresholds for the cross-domain heuristic.
pub struct HeuristicConfig {
    /// Minimum cosine similarity to accept a vector hit (default: 0.72).
    pub min_cosine: f64,
    /// Maximum days between entity creation timestamps (default: 7).
    pub max_temporal_days: i64,
    /// Minimum co-occurrence mention count (default: 2).
    pub min_frequency_mentions: u32,
    /// Minimum number of features the pair appears in together (default: 2).
    pub min_frequency_features: u32,
    /// Minimum number of layers that must match before reporting a dot (default: 2).
    pub min_layers: usize,
}

impl Default for HeuristicConfig {
    fn default() -> Self {
        Self {
            min_cosine: 0.72,
            max_temporal_days: 7,
            min_frequency_mentions: 2,
            min_frequency_features: 2,
            min_layers: 2,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns the canonical lowercase string name for a domain.
pub fn domain_str(domain: &EntityDomain) -> &'static str {
    match domain {
        EntityDomain::Task => "task",
        EntityDomain::Note => "note",
        EntityDomain::Productivity => "productivity",
    }
}

/// The main cross-domain heuristic.
///
/// Evaluates all vector hits against the source entity and returns the single
/// highest-confidence `CrossDomainDot` that satisfies the minimum layer
/// requirement, or `None` if no candidate qualifies.
///
/// # Rules
/// 1. Short / empty source content is ignored (`source_has_content == false`).
/// 2. Only vector hits with `cosine >= config.min_cosine` are considered.
/// 3. Each qualifying hit accumulates up to three evidence layers:
///    - **Semantic overlap** — always granted when the cosine threshold is met.
///    - **Temporal proximity** — granted when the two entities were created
///      within `max_temporal_days` of each other.
///    - **Frequency signal** — granted when `frequency_data` contains an entry
///      for the target entity id with sufficient counts.
/// 4. A hit must accumulate `>= min_layers` to be eligible.
/// 5. The eligible hit with the highest confidence score is returned.
pub fn evaluate_cross_domain(
    input: &HeuristicInput,
    config: &HeuristicConfig,
) -> Option<CrossDomainDot> {
    if !input.source_has_content {
        return None;
    }

    let mut best: Option<CrossDomainDot> = None;

    for (target, cosine, target_created) in &input.vector_hits {
        if *cosine < config.min_cosine {
            continue;
        }

        let mut layers: Vec<Layer> = Vec::new();

        // Layer 1 — semantic overlap (always granted above the cosine threshold).
        layers.push(Layer::SemanticOverlap { cosine: *cosine });

        // Layer 2 — temporal proximity.
        let days_apart = (input.source_created.as_millisecond() - target_created.as_millisecond())
            .abs()
            / 86_400_000;
        if days_apart <= config.max_temporal_days {
            layers.push(Layer::TemporalProximity { days_apart });
        }

        // Layer 3 — frequency signal.
        if let Some((_, mentions, features)) = input
            .frequency_data
            .iter()
            .find(|(id, _, _)| id == &target.id)
        {
            if *mentions >= config.min_frequency_mentions
                && *features >= config.min_frequency_features
            {
                layers.push(Layer::FrequencySignal {
                    mentions: *mentions,
                    features: *features,
                });
            }
        }

        if layers.len() < config.min_layers {
            continue;
        }

        let confidence = compute_confidence(&layers, *cosine);

        let should_replace = best
            .as_ref()
            .map(|b| confidence > b.confidence)
            .unwrap_or(true);

        if should_replace {
            let tooltip = build_tooltip(&input.source, target);
            let detail_route = format!("/{}/{}", domain_str(&target.domain), target.id);
            best = Some(CrossDomainDot {
                source: input.source.clone(),
                target: target.clone(),
                layers_matched: layers,
                confidence,
                tooltip,
                detail_route,
            });
        }
    }

    best
}

/// Generates a context-aware tooltip string for a cross-domain connection.
///
/// The template is chosen based on the ordered pair of source / target domains.
/// Falls back to a generic template when no specific template is defined.
pub fn build_tooltip(source: &EntityRef, target: &EntityRef) -> String {
    use EntityDomain::*;

    match (&source.domain, &target.domain) {
        // Task → Note: research angle
        (Task, Note) => format!(
            "You wrote about this topic in a note — want me to pull \"{}\"?",
            target.title
        ),
        // Task → Productivity: behaviour pattern
        (Task, Productivity) => {
            "Last time you had a similar task you slipped — want me to block focus time?"
                .to_string()
        }
        // Default fallback
        _ => format!(
            "I noticed a connection between \"{}\" and \"{}\"",
            source.title, target.title
        ),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Derives a 0..1 confidence score from accumulated layers and the raw cosine.
///
/// Each additional layer beyond the first contributes a fixed bonus so that
/// three-layer matches score significantly higher than two-layer matches.
fn compute_confidence(layers: &[Layer], cosine: f64) -> f64 {
    // Base score: the cosine similarity (already >= min_cosine).
    let mut score = cosine;

    // Each extra layer beyond the semantic one adds a 0.05 bonus.
    let bonus_layers = layers.len().saturating_sub(1);
    score += bonus_layers as f64 * 0.05;

    score.min(1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    fn utc(year: i16, month: i8, day: i8) -> Timestamp {
        jiff::civil::date(year, month, day)
            .at(0, 0, 0, 0)
            .in_tz("UTC")
            .unwrap()
            .timestamp()
    }

    fn default_config() -> HeuristicConfig {
        HeuristicConfig::default()
    }

    fn task_ref(id: &str, title: &str) -> EntityRef {
        EntityRef {
            domain: EntityDomain::Task,
            id: id.to_string(),
            title: title.to_string(),
        }
    }


    fn note_ref(id: &str, title: &str) -> EntityRef {
        EntityRef {
            domain: EntityDomain::Note,
            id: id.to_string(),
            title: title.to_string(),
        }
    }

    fn productivity_ref(id: &str, title: &str) -> EntityRef {
        EntityRef {
            domain: EntityDomain::Productivity,
            id: id.to_string(),
            title: title.to_string(),
        }
    }

    // Test 1: semantic + temporal layers match → returns dot
    #[test]
    fn cross_domain_two_layers_returns_dot() {
        let source = task_ref("t1", "Q1 budget review");
        let target = note_ref("n1", "AWS invoice March");

        let input = HeuristicInput {
            source: source.clone(),
            source_created: utc(2026, 3, 1),
            vector_hits: vec![(target.clone(), 0.85, utc(2026, 3, 3))], // 2 days apart
            frequency_data: vec![],
            source_has_content: true,
        };

        let result = evaluate_cross_domain(&input, &default_config());
        assert!(result.is_some(), "expected a dot with two layers");
        let dot = result.unwrap();
        assert_eq!(dot.layers_matched.len(), 2);
        assert!(matches!(
            dot.layers_matched[0],
            Layer::SemanticOverlap { .. }
        ));
        assert!(matches!(
            dot.layers_matched[1],
            Layer::TemporalProximity { .. }
        ));
    }

    // Test 2: only one layer (semantic only, temporal too far, no frequency) → returns None
    #[test]
    fn cross_domain_one_layer_returns_none() {
        let source = task_ref("t1", "Q1 budget review");
        let target = note_ref("n1", "AWS invoice March");

        let input = HeuristicInput {
            source: source.clone(),
            source_created: utc(2026, 1, 1),
            vector_hits: vec![(target.clone(), 0.80, utc(2026, 3, 20))], // 78 days apart
            frequency_data: vec![],
            source_has_content: true,
        };

        let result = evaluate_cross_domain(&input, &default_config());
        assert!(result.is_none(), "should require at least 2 layers");
    }

    // Test 3: cosine below threshold → returns None
    #[test]
    fn cross_domain_low_cosine_returns_none() {
        let source = task_ref("t1", "Q1 budget review");
        let target = note_ref("n1", "AWS invoice March");

        let input = HeuristicInput {
            source: source.clone(),
            source_created: utc(2026, 3, 1),
            vector_hits: vec![(target.clone(), 0.50, utc(2026, 3, 3))],
            frequency_data: vec![],
            source_has_content: true,
        };

        let result = evaluate_cross_domain(&input, &default_config());
        assert!(result.is_none(), "cosine 0.50 is below min_cosine 0.72");
    }

    // Test 4: source_has_content = false → returns None immediately
    #[test]
    fn cross_domain_no_content_returns_none() {
        let source = task_ref("t1", "");
        let target = note_ref("n1", "AWS invoice March");

        let input = HeuristicInput {
            source: source.clone(),
            source_created: utc(2026, 3, 1),
            vector_hits: vec![(target.clone(), 0.90, utc(2026, 3, 2))],
            frequency_data: vec![],
            source_has_content: false,
        };

        let result = evaluate_cross_domain(&input, &default_config());
        assert!(result.is_none(), "empty source should be skipped");
    }

    // Test 5: all three layers match → dot with 3 layers
    #[test]
    fn cross_domain_three_layers_returns_dot() {
        let source = task_ref("t1", "Q1 budget review");
        let target = note_ref("n1", "AWS invoice March");

        let input = HeuristicInput {
            source: source.clone(),
            source_created: utc(2026, 3, 1),
            vector_hits: vec![(target.clone(), 0.88, utc(2026, 3, 3))], // 2 days
            frequency_data: vec![("n1".to_string(), 3, 2)],             // meets thresholds
            source_has_content: true,
        };

        let result = evaluate_cross_domain(&input, &default_config());
        assert!(result.is_some(), "all three layers should produce a dot");
        let dot = result.unwrap();
        assert_eq!(dot.layers_matched.len(), 3);
        assert!(matches!(
            dot.layers_matched[2],
            Layer::FrequencySignal { .. }
        ));
        // Three-layer confidence should exceed two-layer
        assert!(dot.confidence > 0.88 + 0.05);
    }

    // Test 6: tooltip template for task → note
    #[test]
    fn cross_domain_tooltip_task_note() {
        let source = task_ref("t1", "Q1 budget review");
        let target = note_ref("n1", "AWS invoice March");

        let tooltip = build_tooltip(&source, &target);
        assert!(
            tooltip.contains("pull \"AWS invoice March\""),
            "tooltip should use task→note template"
        );
    }

    // Bonus test 7: best-hit selection — higher cosine among two eligible candidates wins
    #[test]
    fn cross_domain_returns_best_hit() {
        let source = task_ref("t1", "Q1 budget review");
        let target_a = productivity_ref("p1", "AWS invoice");
        let target_b = note_ref("n1", "Budget notes");

        let input = HeuristicInput {
            source: source.clone(),
            source_created: utc(2026, 3, 5),
            vector_hits: vec![
                (target_a.clone(), 0.75, utc(2026, 3, 4)), // 2 layers: semantic + temporal
                (target_b.clone(), 0.92, utc(2026, 3, 6)), // 2 layers: semantic + temporal, higher cosine
            ],
            frequency_data: vec![],
            source_has_content: true,
        };

        let result = evaluate_cross_domain(&input, &default_config());
        assert!(result.is_some());
        let dot = result.unwrap();
        assert_eq!(dot.target.id, "n1", "higher-cosine hit should be selected");
    }
}
