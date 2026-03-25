use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyStatPoint {
    pub date: String,
    pub reviews: i64,
    pub atoms_created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatsSummaryResponse {
    pub streak: u32,
    pub retention: f64,
    pub weekly: Vec<WeeklyStatPoint>,
}
