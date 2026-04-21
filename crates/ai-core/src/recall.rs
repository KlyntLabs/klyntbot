use crate::RecallDomain;

/// Query the retrieval layer uses to ask each feature "are you relevant?"
#[derive(Debug, Clone)]
pub struct RecallQuery {
    pub message: String,
    pub intent_summary: Option<String>,
}

/// A candidate the retrieval layer considers for prompt injection.
#[derive(Debug, Clone)]
pub struct RecallItem {
    pub id: String,
    pub text: String,
    pub score: f64,
    pub domain: RecallDomain,
}
