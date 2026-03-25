use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MorningBriefingResponse {
    pub streak_days: usize,
    pub due_cards: i64,
    pub fading_atoms: Vec<FadingAtomSummary>,
    pub strongest_topic: Option<TopicSummary>,
    pub weakest_topic: Option<TopicSummary>,
    pub atoms_reviewed_this_week: i64,
    pub atoms_created_this_week: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FadingAtomSummary {
    pub id: String,
    pub subject: String,
    pub retention_pct: f64,
    pub domain: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopicSummary {
    pub name: String,
    pub avg_retention: f64,
    pub atom_count: i64,
}
