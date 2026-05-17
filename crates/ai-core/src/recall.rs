use crate::RecallDomain;

/// Query the retrieval layer uses to ask each feature "are you relevant?"
#[derive(Debug, Clone)]
pub struct RecallQuery {
    pub message: String,
}

/// A candidate the retrieval layer considers for prompt injection.
#[derive(Debug, Clone)]
pub struct RecallItem {
    pub id: String,
    pub text: String,
    pub score: f64,
    pub domain: RecallDomain,
}

/// Per-feature recall configuration emitted by `#[derive(AiFeature)]`.
#[derive(Debug, Clone, Copy)]
pub struct RecallSpec {
    pub priority_field: Option<&'static str>,
    pub recency_field: Option<&'static str>,
    pub status_filter: Option<&'static str>,
}
