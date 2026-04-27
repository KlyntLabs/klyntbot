use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RetentionHistoryParams {
    pub days: i64,
    pub by_domain: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RetentionHistoryResponse {
    pub overall: Vec<RetentionPoint>,
    pub domains: Vec<DomainHistory>,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPoint {
    pub date: String,
    pub avg_retention: f64,
    pub review_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DomainHistory {
    pub domain: String,
    pub points: Vec<RetentionPoint>,
}
