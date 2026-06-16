//! KCA Track 12 — memory-grounded skill discovery.

use cognitive_memory::repos::procedural_rule::ProceduralRuleRepo;

/// A cluster of related procedural rules that may form a skill.
#[derive(Debug, Clone)]
pub struct RuleCluster {
    /// IDs of rules in this cluster.
    pub rule_ids: Vec<String>,
    /// Keywords shared across all rules in the cluster.
    pub shared_keywords: Vec<String>,
    /// Average confidence of rules in the cluster.
    pub avg_confidence: f64,
}

/// Cluster rules by shared keywords (Jaccard similarity ≥ threshold).
pub async fn cluster_rules_for_skill_discovery(
    repo: &ProceduralRuleRepo,
    jaccard_threshold: f64,
) -> common::Result<Vec<RuleCluster>> {
    let rules = repo
        .list_all_active()
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    if rules.len() < 2 {
        return Ok(Vec::new());
    }

    let kw: Vec<std::collections::HashSet<String>> = rules
        .iter()
        .map(|r| keywords_from(&r.rule_text).into_iter().collect())
        .collect();

    // Single-pass clustering: greedy union-find.
    let n = rules.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let a = &kw[i];
            let b = &kw[j];
            if a.is_empty() || b.is_empty() {
                continue;
            }
            let inter = a.intersection(b).count() as f64;
            let union_size = a.union(b).count() as f64;
            let jaccard = inter / union_size;
            if jaccard >= jaccard_threshold {
                union(&mut parent, i, j);
            }
        }
    }

    let mut by_root: std::collections::HashMap<usize, Vec<usize>> = Default::default();
    for i in 0..n {
        let r = find(&mut parent, i);
        by_root.entry(r).or_default().push(i);
    }

    Ok(by_root
        .into_values()
        .filter_map(|idxs| {
            if idxs.len() < 2 {
                return None;
            }
            let mut shared: std::collections::HashSet<String> = kw[idxs[0]].clone();
            for &i in &idxs[1..] {
                shared = shared.intersection(&kw[i]).cloned().collect();
            }
            let conf: f64 =
                idxs.iter().map(|&i| rules[i].confidence).sum::<f64>() / idxs.len() as f64;
            Some(RuleCluster {
                rule_ids: idxs.iter().map(|&i| rules[i].id.clone()).collect(),
                shared_keywords: shared.into_iter().collect(),
                avg_confidence: conf,
            })
        })
        .collect())
}

fn keywords_from(s: &str) -> Vec<String> {
    let stop = [
        "with", "from", "this", "that", "have", "your", "their", "after", "before", "when", "then",
        "user", "always", "should", "would", "into",
    ];
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4 && !stop.contains(w))
        .map(|w| w.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognitive_memory::repos::procedural_rule::ProceduralRuleRepo;
    use cognitive_memory::types::ProceduralRule;

    #[tokio::test]
    async fn clusters_rules_with_shared_keywords() {
        let pool = cognitive_schema::cognitive_test_pool().await;
        let rule_repo = ProceduralRuleRepo::new(pool.clone());

        for (i, text) in [
            "When user runs cargo nextest, also run clippy",
            "After cargo nextest passes, suggest clippy",
            "If clippy fails, run cargo fmt before retry",
            "Track tasks via the tasks tool",
        ]
        .iter()
        .enumerate()
        {
            rule_repo
                .upsert(&ProceduralRule {
                    id: format!("r{i}"),
                    domain: "coding".into(),
                    rule_text: (*text).into(),
                    confidence: 0.85,
                    signal_count: 5,
                    source: "reflected".into(),
                    active: true,
                    ..Default::default()
                })
                .await
                .unwrap();
        }

        let clusters = cluster_rules_for_skill_discovery(&rule_repo, 0.4)
            .await
            .unwrap();
        assert!(!clusters.is_empty());
        // r0+r1 share "cargo|clippy|nextest" keywords; r2 has only cargo+clippy overlap (too low
        // at threshold 0.4) and r3 should not be in any cluster.
        let big_cluster = clusters.iter().find(|c| c.rule_ids.len() >= 2);
        assert!(
            big_cluster.is_some(),
            "expected a 2-rule cluster, got {:?}",
            clusters.iter().map(|c| &c.rule_ids).collect::<Vec<_>>()
        );
    }
}
