use crate::AppCore;
use common::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecallStats {
    pub total_invocations: u64,
    pub mean_latency_ms: f64,
    pub top_facts: Vec<TopFact>,
    pub days_window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TopFact {
    pub fact_id: String,
    pub subject: String,
    pub predicate: String,
    pub recall_count: u64,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_recall_stats(
        &self,
        workspace_id: &str,
        days: Option<u32>,
    ) -> Result<RecallStats> {
        let window = days.unwrap_or(7);
        let _repo = &self.repos;

        // TODO: wire up recall_invocations repo once coding-memory telemetry is
        // integrated into AppCore repos.
        Ok(RecallStats {
            total_invocations: 0,
            mean_latency_ms: 0.0,
            top_facts: vec![],
            days_window: window,
        })
    }
}
