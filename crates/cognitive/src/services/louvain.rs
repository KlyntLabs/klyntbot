//! Louvain community detection algorithm over weighted undirected graphs.

use petgraph::graph::{NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

/// Result of Louvain community detection.
pub struct CommunityAssignment {
    /// Map from node ID (String) to community ID (usize).
    pub assignments: HashMap<String, usize>,
    /// Total modularity score.
    pub modularity: f64,
    /// Number of communities found.
    pub community_count: usize,
}

/// Run Louvain community detection on a weighted undirected graph.
///
/// Input: `edges` is a list of (node_a_id, node_b_id, weight).
/// Returns community assignments for each node.
pub fn detect_communities(edges: &[(String, String, f64)]) -> CommunityAssignment {
    let mut graph = UnGraph::<String, f64>::new_undirected();
    let mut node_map: HashMap<String, NodeIndex> = HashMap::new();
    let mut id_map: HashMap<NodeIndex, String> = HashMap::new();

    for (a, b, w) in edges {
        let na = *node_map.entry(a.clone()).or_insert_with(|| {
            let idx = graph.add_node(a.clone());
            id_map.insert(idx, a.clone());
            idx
        });
        let nb = *node_map.entry(b.clone()).or_insert_with(|| {
            let idx = graph.add_node(b.clone());
            id_map.insert(idx, b.clone());
            idx
        });
        graph.add_edge(na, nb, *w);
    }

    if graph.node_count() == 0 {
        return CommunityAssignment {
            assignments: HashMap::new(),
            modularity: 0.0,
            community_count: 0,
        };
    }

    // Total weight of all edges (doubled for undirected)
    let total_weight: f64 = graph.edge_weights().sum::<f64>() * 2.0;
    if total_weight == 0.0 {
        let assignments: HashMap<String, usize> = node_map
            .into_iter()
            .enumerate()
            .map(|(i, (id, _))| (id, i))
            .collect();
        let count = assignments.len();
        return CommunityAssignment {
            assignments,
            modularity: 0.0,
            community_count: count,
        };
    }

    // Initialize: each node in its own community
    let mut community: HashMap<NodeIndex, usize> = graph
        .node_indices()
        .enumerate()
        .map(|(i, n)| (n, i))
        .collect();

    // Precompute node degrees (k_i = sum of edge weights for node i)
    let degrees: HashMap<NodeIndex, f64> = graph
        .node_indices()
        .map(|n| (n, graph.edges(n).map(|e| *e.weight()).sum()))
        .collect();

    // Incremental Σ_tot per community (sum of degrees of all members)
    let mut sigma_tot: HashMap<usize, f64> = HashMap::new();
    for n in graph.node_indices() {
        *sigma_tot.entry(community[&n]).or_insert(0.0) += degrees[&n];
    }

    // m = sum of all edge weights (half of total_weight)
    let m = total_weight / 2.0;

    // Louvain Phase 1: local moves
    let mut improved = true;
    while improved {
        improved = false;
        for node in graph.node_indices() {
            let current_comm = community[&node];
            let k_i = degrees[&node];

            // Collect unique neighbor communities
            let mut neighbor_comms: HashSet<usize> = HashSet::new();
            for n in graph.neighbors(node) {
                neighbor_comms.insert(community[&n]);
            }

            // k_{i,in} for current community
            let k_i_in_current: f64 = graph
                .edges(node)
                .filter(|e| {
                    let other = if e.source() == node {
                        e.target()
                    } else {
                        e.source()
                    };
                    community[&other] == current_comm
                })
                .map(|e| *e.weight())
                .sum();

            // Σ_tot for current community excluding this node (O(1) lookup)
            let sigma_tot_current = sigma_tot.get(&current_comm).copied().unwrap_or(0.0) - k_i;

            let remove_cost = k_i_in_current / m - (sigma_tot_current * k_i) / (2.0 * m * m);

            let mut best_comm = current_comm;
            let mut best_gain = 0.0_f64;

            for &target_comm in &neighbor_comms {
                if target_comm == current_comm {
                    continue;
                }

                // k_{i,in} for target community
                let k_i_in_target: f64 = graph
                    .edges(node)
                    .filter(|e| {
                        let other = if e.source() == node {
                            e.target()
                        } else {
                            e.source()
                        };
                        community[&other] == target_comm
                    })
                    .map(|e| *e.weight())
                    .sum();

                // Σ_tot for target community (O(1) lookup)
                let sigma_tot_target = sigma_tot.get(&target_comm).copied().unwrap_or(0.0);

                let add_gain = k_i_in_target / m - (sigma_tot_target * k_i) / (2.0 * m * m);
                let gain = add_gain - remove_cost;

                if gain > best_gain {
                    best_gain = gain;
                    best_comm = target_comm;
                }
            }

            if best_comm != current_comm {
                // Update sigma_tot incrementally
                *sigma_tot.entry(current_comm).or_insert(0.0) -= k_i;
                *sigma_tot.entry(best_comm).or_insert(0.0) += k_i;
                community.insert(node, best_comm);
                improved = true;
            }
        }
    }

    // Renumber communities to 0..N
    let mut comm_renumber: HashMap<usize, usize> = HashMap::new();
    let mut next_id = 0;
    let assignments: HashMap<String, usize> = graph
        .node_indices()
        .map(|n| {
            let comm = community[&n];
            let new_comm = *comm_renumber.entry(comm).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            (id_map[&n].clone(), new_comm)
        })
        .collect();

    let modularity = calculate_modularity(&graph, &community, &degrees, total_weight);

    CommunityAssignment {
        assignments,
        modularity,
        community_count: next_id,
    }
}

fn calculate_modularity(
    graph: &UnGraph<String, f64>,
    community: &HashMap<NodeIndex, usize>,
    degrees: &HashMap<NodeIndex, f64>,
    total_weight: f64,
) -> f64 {
    let mut q = 0.0;
    for edge in graph.edge_references() {
        let (a, b) = (edge.source(), edge.target());
        if community[&a] == community[&b] {
            let w = *edge.weight();
            let ka = degrees[&a];
            let kb = degrees[&b];
            q += w - (ka * kb) / total_weight;
        }
    }
    q / total_weight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let result = detect_communities(&[]);
        assert_eq!(result.community_count, 0);
        assert!(result.assignments.is_empty());
    }

    #[test]
    fn test_single_edge() {
        let edges = vec![("a".into(), "b".into(), 1.0)];
        let result = detect_communities(&edges);
        assert!(result.community_count >= 1);
        assert_eq!(result.assignments["a"], result.assignments["b"]);
    }

    #[test]
    fn test_two_clusters() {
        let edges = vec![
            // Cluster 1: tight
            ("a".into(), "b".into(), 1.0),
            ("b".into(), "c".into(), 1.0),
            ("a".into(), "c".into(), 1.0),
            // Cluster 2: tight
            ("d".into(), "e".into(), 1.0),
            ("e".into(), "f".into(), 1.0),
            ("d".into(), "f".into(), 1.0),
            // Weak bridge between clusters
            ("c".into(), "d".into(), 0.1),
        ];
        let result = detect_communities(&edges);
        assert!(result.community_count >= 2);
        assert_eq!(result.assignments["a"], result.assignments["b"]);
        assert_eq!(result.assignments["b"], result.assignments["c"]);
        assert_eq!(result.assignments["d"], result.assignments["e"]);
        assert_eq!(result.assignments["e"], result.assignments["f"]);
        assert_ne!(result.assignments["a"], result.assignments["d"]);
    }

    #[test]
    fn test_modularity_positive() {
        let edges = vec![
            ("a".into(), "b".into(), 1.0),
            ("b".into(), "c".into(), 1.0),
            ("a".into(), "c".into(), 1.0),
        ];
        let result = detect_communities(&edges);
        assert!(result.modularity >= 0.0);
    }
}
