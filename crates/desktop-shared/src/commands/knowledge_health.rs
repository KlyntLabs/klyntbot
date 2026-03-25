use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeHealthSummary {
    pub total_atoms: usize,
    pub active_atoms: usize,
    pub avg_retention: f64,
    pub topics: Vec<TopicHealthResponse>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopicHealthResponse {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub atom_count: i64,
    pub avg_retention: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetailParams {
    pub topic_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopicDetailResponse {
    pub topic: TopicHealthResponse,
    pub atoms: Vec<super::KnowledgeAtomResponse>,
}
