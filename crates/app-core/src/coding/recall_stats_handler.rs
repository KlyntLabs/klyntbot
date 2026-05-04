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
        let repo = &self.repos;

        let count = repo
            .recall_invocations
            .count_in_last_days(workspace_id, window)
            .await?;
        let mean_latency = repo
            .recall_invocations
            .mean_latency_in_last_days(workspace_id, window)
            .await?;
        let top = repo
            .recall_invocations
            .top_facts_in_last_days(workspace_id, window, 5)
            .await?;

        Ok(RecallStats {
            total_invocations: count,
            mean_latency_ms: mean_latency,
            top_facts: top
                .into_iter()
                .map(|r| TopFact {
                    fact_id: r.fact_id,
                    subject: r.subject,
                    predicate: r.predicate,
                    recall_count: r.recall_count,
                })
                .collect(),
            days_window: window,
        })
    }
}
