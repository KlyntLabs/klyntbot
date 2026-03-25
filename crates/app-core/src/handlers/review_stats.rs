use desktop_shared::commands::{ReviewStatsSummaryResponse, WeeklyStatPoint};
use desktop_shared::errors::ApiError;

use super::atoms::map_db;
use crate::state::AppCore;

impl AppCore {
    pub async fn review_stats_summary(&self) -> Result<ReviewStatsSummaryResponse, ApiError> {
        let repo = cognitive::ReviewStatsRepo::new(self.storage_pool.inner().clone());

        let (streak, retention, daily_reviews, daily_atoms) = tokio::try_join!(
            repo.current_streak(),
            repo.knowledge_retention_score(),
            repo.daily_reviews(7),
            repo.daily_atoms_created(7),
        )
        .map_err(map_db)?;

        let today = chrono::Utc::now().date_naive();
        let mut weekly = Vec::with_capacity(7);
        for i in (0..7).rev() {
            let date = (today - chrono::Duration::days(i))
                .format("%Y-%m-%d")
                .to_string();
            let reviews = daily_reviews
                .iter()
                .find(|r| r.date == date)
                .map(|r| r.review_count)
                .unwrap_or(0);
            let atoms = daily_atoms
                .iter()
                .find(|(d, _)| d == &date)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            weekly.push(WeeklyStatPoint {
                date,
                reviews,
                atoms_created: atoms,
            });
        }

        Ok(ReviewStatsSummaryResponse {
            streak: streak as u32,
            retention,
            weekly,
        })
    }
}
