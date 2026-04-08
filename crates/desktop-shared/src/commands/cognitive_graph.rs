//! Response types for the cognitive graph visualization.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicNode {
    pub id: String,
    pub subject: String,
    pub domain: String,
    pub fact_count: u32,
    pub avg_convergence: f64,
    pub max_confidence: f64,
    pub total_access_count: i64,
    pub last_accessed: Option<String>,
    pub community_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicEdge {
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveCommunity {
    pub id: String,
    pub name: String,
    pub color: String,
    pub member_topic_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleNode {
    pub id: String,
    pub rule_text: String,
    pub domain: String,
    pub signal_count: i64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStats {
    pub total_facts: u32,
    pub total_topics: u32,
    pub total_edges: u32,
    pub total_communities: u32,
    pub avg_convergence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveGraphData {
    pub topics: Vec<TopicNode>,
    pub edges: Vec<TopicEdge>,
    pub communities: Vec<CognitiveCommunity>,
    pub rules: Vec<RuleNode>,
    pub stats: GraphStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactNode {
    pub id: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub convergence_score: f64,
    pub stability: f64,
    pub access_count: i64,
    pub source: String,
    pub last_accessed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetail {
    pub topic_id: String,
    pub facts: Vec<FactNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicExpandParams {
    pub subject: String,
    pub domain: String,
}
