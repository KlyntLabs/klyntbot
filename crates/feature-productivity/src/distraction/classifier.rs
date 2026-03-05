//! Trait for async content classification (LLM-backed).

use async_trait::async_trait;

/// Classification result from the LLM.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentClassification {
    Educational,
    WorkResearch,
    Entertainment,
    SocialMedia,
    Unknown,
}

impl std::fmt::Display for ContentClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Educational => write!(f, "Likely educational content"),
            Self::WorkResearch => write!(f, "Likely work-related research"),
            Self::Entertainment => write!(f, "Entertainment detected"),
            Self::SocialMedia => write!(f, "Social media detected"),
            Self::Unknown => write!(f, "Unable to classify"),
        }
    }
}

#[async_trait]
pub trait DistractionClassifierHandler: Send + Sync {
    /// Classify the content at the given window title.
    /// Should complete within the configured timeout.
    async fn classify(
        &self,
        app_name: &str,
        window_title: &str,
    ) -> common::Result<ContentClassification>;
}
