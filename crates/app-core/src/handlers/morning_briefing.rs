use desktop_shared::commands::{FadingAtomSummary, MorningBriefingResponse, TopicSummary};
use desktop_shared::errors::ApiError;

use super::atoms::map_db;
use crate::state::AppCore;

impl AppCore {
    pub async fn morning_briefing(&self) -> Result<MorningBriefingResponse, ApiError> {
        let atom_repo = self.knowledge_atom_repo()?;
        let fc_repo = self.flashcard_repo()?;

        let review_stats = cognitive::ReviewStatsRepo::new(atom_repo.pool().clone());
        let week_ago = jiff::Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(7 * 86400))
            .unwrap_or_else(|_| jiff::Timestamp::now())
            .to_string();

        // Run all independent queries concurrently
        let (streak_res, due_res, fading_res, topics_res, daily_res, created_res) = tokio::join!(
            review_stats.current_streak(),
            fc_repo.total_due_count(),
            atom_repo.list_fading_important(5),
            atom_repo.list_topics_with_atoms(),
            review_stats.daily_reviews(7),
            atom_repo.count_created_since(&week_ago),
        );

        let streak = streak_res.unwrap_or(0);
        let due = due_res.map_err(map_db)?;
        let fading: Vec<FadingAtomSummary> = fading_res
            .map_err(map_db)?
            .iter()
            .map(|a| FadingAtomSummary {
                id: a.id.clone(),
                subject: a.subject.clone(),
                retention_pct: a.retention_pct,
                domain: a.domain.clone(),
            })
            .collect();

        // Topics (sorted by avg_retention ASC, so first = weakest, last = strongest)
        let topics = topics_res.unwrap_or_default();
        let weakest = topics.first().map(|t| TopicSummary {
            name: t.name.clone(),
            avg_retention: t.avg_retention,
            atom_count: t.atom_count,
        });
        let strongest = topics.last().map(|t| TopicSummary {
            name: t.name.clone(),
            avg_retention: t.avg_retention,
            atom_count: t.atom_count,
        });

        // Weekly review count
        let daily = daily_res.unwrap_or_default();
        let reviews_week: i64 = daily.iter().map(|d| d.review_count).sum();

        let created_week = created_res.unwrap_or(0);

        Ok(MorningBriefingResponse {
            streak_days: streak,
            due_cards: due,
            fading_atoms: fading,
            strongest_topic: strongest,
            weakest_topic: weakest,
            atoms_reviewed_this_week: reviews_week,
            atoms_created_this_week: created_week,
        })
    }
}
