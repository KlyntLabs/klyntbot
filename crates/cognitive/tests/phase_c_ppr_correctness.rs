//! Personalized PageRank correctness — KCA Track 6.

use cognitive::services::ppr_retrieval::{personalized_pagerank, PprConfig};
use petgraph::graph::DiGraph;

#[test]
fn ppr_concentrates_mass_on_seed_neighborhood() {
    // Star graph: A in center, B,C,D leaves; E disconnected.
    let mut g: DiGraph<String, f32> = DiGraph::new();
    let a = g.add_node("A".into());
    let b = g.add_node("B".into());
    let c = g.add_node("C".into());
    let d = g.add_node("D".into());
    let _e = g.add_node("E".into());

    g.add_edge(a, b, 1.0);
    g.add_edge(b, a, 1.0);
    g.add_edge(a, c, 1.0);
    g.add_edge(c, a, 1.0);
    g.add_edge(a, d, 1.0);
    g.add_edge(d, a, 1.0);
    // E is disconnected.

    let scores = personalized_pagerank(&g, &[a], &PprConfig::default());

    // Disconnected E should have ~0 score.
    let s_e = scores.get(&"E".to_string()).copied().unwrap_or(0.0);
    assert!(s_e < 0.01, "disconnected E score = {s_e}");

    // A should have the highest score (it's the seed and the hub).
    let s_a = scores.get(&"A".to_string()).copied().unwrap_or(0.0);
    let s_b = scores.get(&"B".to_string()).copied().unwrap_or(0.0);
    assert!(s_a > s_b, "seed should outrank neighbor");

    // B, C, D should have nontrivial scores.
    assert!(s_b > 0.05);

    // Sum of scores ≈ 1.0 (probability distribution).
    let total: f32 = scores.values().sum();
    assert!(
        (total - 1.0).abs() < 0.05,
        "scores sum should be ~1.0, got {total}"
    );
}

#[test]
fn ppr_handles_empty_seeds_returns_uniform_or_empty() {
    let mut g: DiGraph<String, f32> = DiGraph::new();
    let _a = g.add_node("A".into());
    let _b = g.add_node("B".into());
    let scores = personalized_pagerank(&g, &[], &PprConfig::default());
    assert!(scores.is_empty());
}

#[test]
fn ppr_respects_alpha_teleportation_probability() {
    // Two-node graph with one cycle.
    let mut g: DiGraph<String, f32> = DiGraph::new();
    let a = g.add_node("A".into());
    let b = g.add_node("B".into());
    g.add_edge(a, b, 1.0);
    g.add_edge(b, a, 1.0);

    let strict_seed = PprConfig {
        alpha: 0.05,
        max_iterations: 50,
        tolerance: 1e-6,
    };
    let exploratory = PprConfig {
        alpha: 0.5,
        max_iterations: 50,
        tolerance: 1e-6,
    };

    let s1 = personalized_pagerank(&g, &[a], &strict_seed);
    let s2 = personalized_pagerank(&g, &[a], &exploratory);

    // Higher alpha = more teleportation back to seed = stronger A bias.
    let bias_strict = s1["A"] - s1["B"];
    let bias_exploratory = s2["A"] - s2["B"];
    assert!(
        bias_exploratory > bias_strict,
        "alpha=0.5 ({}) should bias toward seed more than alpha=0.05 ({})",
        bias_exploratory,
        bias_strict
    );
}
