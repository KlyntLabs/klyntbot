use std::collections::HashMap;

use async_trait::async_trait;
use common::Result;

use super::{Operator, OperatorContext, OperatorType};
use crate::book_index::ScoredNode;

/// Personalized PageRank on a subgraph seeded from query entities.
pub fn pagerank_scores(
    node_ids: &[&str],
    edges: &[(&str, &str, f64)],
    seed_ids: &[&str],
    damping: f64,
    iterations: u32,
) -> HashMap<String, f64> {
    let n = node_ids.len();
    if n == 0 {
        return HashMap::new();
    }

    let id_to_idx: HashMap<&str, usize> = node_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    // Build adjacency (outgoing edges with weights)
    let mut out_weights: Vec<Vec<(usize, f64)>> = vec![vec![]; n];
    let mut out_totals: Vec<f64> = vec![0.0; n];
    for &(from, to, weight) in edges {
        if let (Some(&fi), Some(&ti)) = (id_to_idx.get(from), id_to_idx.get(to)) {
            out_weights[fi].push((ti, weight));
            out_totals[fi] += weight;
        }
    }

    // Personalization vector
    let mut personalization = vec![0.0; n];
    let seed_count = seed_ids
        .iter()
        .filter(|s| id_to_idx.contains_key(**s))
        .count();
    if seed_count > 0 {
        let seed_val = 1.0 / seed_count as f64;
        for &sid in seed_ids {
            if let Some(&idx) = id_to_idx.get(sid) {
                personalization[idx] = seed_val;
            }
        }
    } else {
        let uniform = 1.0 / n as f64;
        personalization.fill(uniform);
    }

    // Initialize scores
    let mut scores = personalization.clone();

    // Iterate
    for _ in 0..iterations {
        let mut new_scores = vec![0.0; n];
        let mut dangling_mass = 0.0;
        for i in 0..n {
            if out_totals[i] > 0.0 {
                for &(j, w) in &out_weights[i] {
                    new_scores[j] += damping * scores[i] * w / out_totals[i];
                }
            } else {
                // Dangling node: redistribute mass to personalization vector
                dangling_mass += scores[i];
            }
        }
        // Distribute dangling mass + teleportation
        for i in 0..n {
            new_scores[i] += damping * dangling_mass * personalization[i];
            new_scores[i] += (1.0 - damping) * personalization[i];
        }
        scores = new_scores;
    }

    node_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id.to_string(), scores[i]))
        .collect()
}

/// Retain only non-dominated nodes across (graph_score, text_score).
pub fn skyline_filter(nodes: &[ScoredNode]) -> Vec<ScoredNode> {
    nodes
        .iter()
        .filter(|candidate| {
            !nodes
                .iter()
                .any(|other| !std::ptr::eq(*candidate, other) && dominates(other, candidate))
        })
        .cloned()
        .collect()
}

fn dominates(a: &ScoredNode, b: &ScoredNode) -> bool {
    a.graph_score >= b.graph_score
        && a.text_score >= b.text_score
        && (a.graph_score > b.graph_score || a.text_score > b.text_score)
}

/// GraphReasoning operator: runs PageRank on entity subgraph, maps scores to nodes via GT-Link.
pub struct GraphReasoning;

impl Default for GraphReasoning {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphReasoning {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for GraphReasoning {
    fn name(&self) -> &str {
        "GraphReasoning"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Reasoner
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        // Collect entity IDs from extracted entities
        let entity_ids: Vec<String> = ctx
            .extracted_entities
            .iter()
            .map(|e| e.id.clone())
            .collect();
        if entity_ids.is_empty() {
            return Ok(());
        }

        // Get neighborhood for each entity
        let mut all_node_ids: Vec<String> = entity_ids.clone();
        let mut all_edges: Vec<(String, String, f64)> = Vec::new();

        for eid in &entity_ids {
            if let Ok(neighbors) = ctx
                .book_index
                .entity_repo()
                .get_neighborhood_ids(eid, 1)
                .await
            {
                for (neighbor_id, _rel_type, weight) in neighbors {
                    all_edges.push((eid.clone(), neighbor_id.clone(), weight));
                    if !all_node_ids.contains(&neighbor_id) {
                        all_node_ids.push(neighbor_id);
                    }
                }
            }
        }

        let node_id_refs: Vec<&str> = all_node_ids.iter().map(|s| s.as_str()).collect();
        let edge_refs: Vec<(&str, &str, f64)> = all_edges
            .iter()
            .map(|(a, b, w)| (a.as_str(), b.as_str(), *w))
            .collect();
        let seed_refs: Vec<&str> = entity_ids.iter().map(|s| s.as_str()).collect();

        let pr_scores = pagerank_scores(&node_id_refs, &edge_refs, &seed_refs, 0.85, 20);

        // Map entity scores to tree nodes via GT-Link
        for scored in &mut ctx.working_set {
            // Check if any linked entity has a PageRank score
            if let Ok(entities) = ctx
                .book_index
                .gt_link_repo()
                .get_entities_in_subtree(&scored.node.id)
                .await
            {
                let max_pr = entities
                    .iter()
                    .filter_map(|eid| pr_scores.get(eid))
                    .copied()
                    .fold(0.0_f64, f64::max);
                scored.graph_score = max_pr;
            }
        }

        Ok(())
    }
}

/// TextRanker: embeds query and scores each node by cosine similarity.
pub struct TextRanker;

impl Default for TextRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl TextRanker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for TextRanker {
    fn name(&self) -> &str {
        "TextRanker"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Reasoner
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        let query_vec = ctx.book_index.embedder().embed(&ctx.query).await?;

        for scored in &mut ctx.working_set {
            let node_vec = ctx
                .book_index
                .embedder()
                .embed(&scored.node.content)
                .await?;
            scored.text_score = cosine_similarity(&query_vec, &node_vec);
            // Update combined as weighted average
            scored.combined = 0.5 * scored.graph_score + 0.5 * scored.text_score;
        }

        // Sort by combined score descending
        ctx.working_set.sort_by(|a, b| {
            b.combined
                .partial_cmp(&a.combined)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ctx.working_set.truncate(ctx.max_nodes);

        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a
        .iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b
        .iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// SkylineRanker: Pareto frontier on (graph_score, text_score).
pub struct SkylineRanker;

impl Default for SkylineRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl SkylineRanker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for SkylineRanker {
    fn name(&self) -> &str {
        "SkylineRanker"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Reasoner
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        ctx.working_set = skyline_filter(&ctx.working_set);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagerank_single_node() {
        let scores = pagerank_scores(&["a"], &[], &["a"], 0.85, 20);
        assert!((scores["a"] - 1.0).abs() < 0.01);
    }

    #[test]
    fn pagerank_seeded() {
        let scores = pagerank_scores(
            &["a", "b", "c"],
            &[("a", "b", 1.0), ("b", "c", 1.0)],
            &["a"],
            0.85,
            20,
        );
        assert!(scores["a"] > scores["b"]);
        assert!(scores["b"] > scores["c"]);
    }

    #[test]
    fn skyline_basic() {
        let nodes = vec![
            scored_node("a", 0.9, 0.1),
            scored_node("b", 0.1, 0.9),
            scored_node("c", 0.5, 0.5),
            scored_node("d", 0.1, 0.1),
        ];
        let frontier = skyline_filter(&nodes);
        let ids: Vec<&str> = frontier.iter().map(|n| n.node.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
        assert!(!ids.contains(&"d"));
    }

    fn scored_node(id: &str, gs: f64, ts: f64) -> ScoredNode {
        use crate::book_index::{SourceType, TreeNode, TreeNodeType};
        ScoredNode {
            node: TreeNode {
                id: id.into(),
                parent_id: None,
                node_type: TreeNodeType::Text,
                content: String::new(),
                title: None,
                level: 0,
                source_type: SourceType::Note,
                source_id: String::new(),
                position: 0,
                metadata: None,
            },
            graph_score: gs,
            text_score: ts,
            combined: 0.0,
        }
    }
}
