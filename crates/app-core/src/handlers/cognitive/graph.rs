//! Cognitive graph data handlers — build topic graph from semantic facts + co-activation.

use std::collections::HashMap;

use cognitive::repos::{CoActivationRepo, ProceduralRuleRepo, SemanticFactRepo};
use cognitive::services::louvain;
use desktop_shared::commands::cognitive_graph::*;
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

const COMMUNITY_COLORS: &[&str] = &[
    "#8b5cf6", "#06b6d4", "#f59e0b", "#ef4444", "#22c55e", "#ec4899", "#3b82f6", "#f97316",
];

/// Generate a stable topic ID from subject + domain.
fn topic_id(subject: &str, domain: &str) -> String {
    format!("{}::{}", subject.to_lowercase(), domain)
}

impl AppCore {
    /// Build the full cognitive graph: topics, edges, communities, rules, stats.
    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_graph_data(&self) -> Result<CognitiveGraphData, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let co_repo = CoActivationRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool.clone());

        // 1. Load all active semantic facts.
        let facts = fact_repo
            .list_all_active()
            .await
            .map_err(map_cognitive_err)?;
        let total_facts = facts.len() as u32;

        // 2. Group by (subject_lower, domain) into topics.
        //    Also build a fact_id -> topic_id map for edge mapping.
        struct TopicAccum {
            subject_display: String,
            domain: String,
            fact_count: u32,
            convergence_sum: f64,
            max_confidence: f64,
            total_access_count: i64,
            last_accessed: Option<String>,
        }

        let mut topic_map: HashMap<String, TopicAccum> = HashMap::new();
        let mut fact_to_topic: HashMap<String, String> = HashMap::new();

        for fact in &facts {
            let tid = topic_id(&fact.subject, &fact.domain);
            fact_to_topic.insert(fact.id.clone(), tid.clone());

            let accum = topic_map.entry(tid).or_insert_with(|| TopicAccum {
                subject_display: fact.subject.clone(),
                domain: fact.domain.clone(),
                fact_count: 0,
                convergence_sum: 0.0,
                max_confidence: 0.0,
                total_access_count: 0,
                last_accessed: None,
            });

            accum.fact_count += 1;
            accum.convergence_sum += fact.convergence_score;
            if fact.confidence > accum.max_confidence {
                accum.max_confidence = fact.confidence;
            }
            accum.total_access_count += fact.access_count;

            // Track the most recent last_accessed across all facts in this topic.
            if let Some(ref fa) = fact.last_accessed {
                match &accum.last_accessed {
                    None => accum.last_accessed = Some(fa.clone()),
                    Some(existing) if fa > existing => accum.last_accessed = Some(fa.clone()),
                    _ => {}
                }
            }
        }

        // 3. Build TopicNode vec (without community_id for now).
        let mut topics: Vec<TopicNode> = topic_map
            .iter()
            .map(|(tid, acc)| TopicNode {
                id: tid.clone(),
                subject: acc.subject_display.clone(),
                domain: acc.domain.clone(),
                fact_count: acc.fact_count,
                avg_convergence: if acc.fact_count > 0 {
                    acc.convergence_sum / acc.fact_count as f64
                } else {
                    0.0
                },
                max_confidence: acc.max_confidence,
                total_access_count: acc.total_access_count,
                last_accessed: acc.last_accessed.clone(),
                community_id: None,
            })
            .collect();

        // Sort topics by fact_count desc for stable ordering.
        topics.sort_by_key(|b| std::cmp::Reverse(b.fact_count));

        // 4. Load co-activation pairs and map fact-level pairs to topic-level edges.
        let co_pairs = co_repo.list_all_pairs().await.map_err(map_cognitive_err)?;

        let mut edge_strengths: HashMap<(String, String), f64> = HashMap::new();
        for (fact_a, fact_b, strength) in &co_pairs {
            let topic_a = fact_to_topic.get(fact_a);
            let topic_b = fact_to_topic.get(fact_b);
            if let (Some(ta), Some(tb)) = (topic_a, topic_b) {
                if ta == tb {
                    continue; // skip intra-topic edges
                }
                // Canonical ordering for dedup.
                let key = if ta < tb {
                    (ta.clone(), tb.clone())
                } else {
                    (tb.clone(), ta.clone())
                };
                *edge_strengths.entry(key).or_insert(0.0) += strength;
            }
        }

        let edges: Vec<TopicEdge> = edge_strengths
            .iter()
            .map(|((src, tgt), &strength)| TopicEdge {
                source_topic_id: src.clone(),
                target_topic_id: tgt.clone(),
                strength,
            })
            .collect();

        // 5. Community detection.
        let communities = if !edge_strengths.is_empty() {
            // Louvain on topic-level edges.
            let louvain_edges: Vec<(String, String, f64)> = edge_strengths
                .iter()
                .map(|((a, b), &s)| (a.clone(), b.clone(), s))
                .collect();
            let result = louvain::detect_communities(&louvain_edges);

            // Assign community_id to topics.
            for topic in &mut topics {
                if let Some(&cid) = result.assignments.get(&topic.id) {
                    topic.community_id = Some(format!("c{cid}"));
                }
            }

            // Build community structs.
            let mut community_members: HashMap<usize, Vec<String>> = HashMap::new();
            for (tid, &cid) in &result.assignments {
                community_members.entry(cid).or_default().push(tid.clone());
            }

            let mut communities: Vec<CognitiveCommunity> = community_members
                .into_iter()
                .map(|(cid, member_ids)| {
                    // Auto-name from top 2 subjects in this community.
                    let name = auto_name_community(&member_ids);
                    let color = COMMUNITY_COLORS[cid % COMMUNITY_COLORS.len()].to_string();
                    CognitiveCommunity {
                        id: format!("c{cid}"),
                        name,
                        color,
                        member_topic_ids: member_ids,
                    }
                })
                .collect();
            communities.sort_by(|a, b| a.id.cmp(&b.id));
            communities
        } else {
            // Domain-based fallback: one community per domain.
            let mut domain_groups: HashMap<String, Vec<String>> = HashMap::new();
            for topic in &topics {
                domain_groups
                    .entry(topic.domain.clone())
                    .or_default()
                    .push(topic.id.clone());
            }

            let mut communities: Vec<CognitiveCommunity> = domain_groups
                .into_iter()
                .enumerate()
                .map(|(idx, (domain, member_ids))| {
                    let color = COMMUNITY_COLORS[idx % COMMUNITY_COLORS.len()].to_string();
                    let cid = format!("c{idx}");
                    // Assign community_id to matching topics.
                    for topic in &mut topics {
                        if member_ids.contains(&topic.id) {
                            topic.community_id = Some(cid.clone());
                        }
                    }
                    CognitiveCommunity {
                        id: cid,
                        name: domain,
                        color,
                        member_topic_ids: member_ids,
                    }
                })
                .collect();
            communities.sort_by(|a, b| a.id.cmp(&b.id));
            communities
        };

        // 6. Load procedural rules.
        let rules_raw = rule_repo
            .list_all_active()
            .await
            .map_err(map_cognitive_err)?;
        let rules: Vec<RuleNode> = rules_raw
            .into_iter()
            .map(|r| RuleNode {
                id: r.id,
                rule_text: r.rule_text,
                domain: r.domain,
                signal_count: r.signal_count,
                confidence: r.confidence,
            })
            .collect();

        // 7. Compute stats.
        let global_avg_convergence = if total_facts > 0 {
            facts.iter().map(|f| f.convergence_score).sum::<f64>() / total_facts as f64
        } else {
            0.0
        };

        let stats = GraphStats {
            total_facts,
            total_topics: topics.len() as u32,
            total_edges: edges.len() as u32,
            total_communities: communities.len() as u32,
            avg_convergence: global_avg_convergence,
        };

        Ok(CognitiveGraphData {
            topics,
            edges,
            communities,
            rules,
            stats,
        })
    }

    /// Expand a topic into its constituent facts.
    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_graph_expand_topic(
        &self,
        params: TopicExpandParams,
    ) -> Result<TopicDetail, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        let domain_facts = fact_repo
            .list_active(&params.domain)
            .await
            .map_err(map_cognitive_err)?;

        let subject_lower = params.subject.to_lowercase();
        let matching: Vec<FactNode> = domain_facts
            .into_iter()
            .filter(|f| f.subject.to_lowercase() == subject_lower)
            .map(|f| FactNode {
                id: f.id,
                predicate: f.predicate,
                object: f.object,
                confidence: f.confidence,
                convergence_score: f.convergence_score,
                stability: f.stability,
                access_count: f.access_count,
                source: f.source,
                last_accessed: f.last_accessed,
            })
            .collect();

        Ok(TopicDetail {
            topic_id: topic_id(&params.subject, &params.domain),
            facts: matching,
        })
    }
}

/// Auto-name a community from the top 2 most frequent subjects in its member topics.
fn auto_name_community(member_topic_ids: &[String]) -> String {
    // Parse the topic_id format "subject::domain" to extract subjects.
    let mut subject_counts: HashMap<&str, u32> = HashMap::new();
    for tid in member_topic_ids {
        if let Some(subject) = tid.split("::").next() {
            *subject_counts.entry(subject).or_insert(0) += 1;
        }
    }
    let mut subjects: Vec<(&str, u32)> = subject_counts.into_iter().collect();
    subjects.sort_by_key(|b| std::cmp::Reverse(b.1));

    match subjects.len() {
        0 => "Unknown".to_string(),
        1 => capitalize(subjects[0].0),
        _ => format!(
            "{} & {}",
            capitalize(subjects[0].0),
            capitalize(subjects[1].0)
        ),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}
