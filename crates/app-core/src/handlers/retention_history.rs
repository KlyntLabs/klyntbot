use desktop_shared::commands::{
    DomainHistory, RetentionHistoryParams, RetentionHistoryResponse, RetentionPoint,
};
use desktop_shared::errors::ApiError;

use super::atoms::map_db;
use crate::state::AppCore;

fn point_to_ipc(p: cognitive::DailyRetentionPoint) -> RetentionPoint {
    RetentionPoint {
        date: p.date,
        avg_retention: p.avg_retention,
        review_count: p.review_count,
    }
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn retention_history(
        &self,
        params: RetentionHistoryParams,
    ) -> Result<RetentionHistoryResponse, ApiError> {
        let repo = cognitive::RetentionHistoryRepo::new(self.storage_pool.inner().clone());

        let overall = repo
            .daily_retention(params.days)
            .await
            .map_err(map_db)?
            .into_iter()
            .map(point_to_ipc)
            .collect();

        let domains = if params.by_domain {
            repo.domain_retention_history(params.days)
                .await
                .map_err(map_db)?
                .into_iter()
                .map(|d| DomainHistory {
                    domain: d.domain,
                    points: d.points.into_iter().map(point_to_ipc).collect(),
                })
                .collect()
        } else {
            vec![]
        };

        Ok(RetentionHistoryResponse { overall, domains })
    }
}
