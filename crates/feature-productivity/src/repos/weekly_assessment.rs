use sqlx::SqlitePool;

use crate::types::WeeklyAssessment;

#[derive(Debug, Clone)]
pub struct WeeklyAssessmentRepo {
    pool: SqlitePool,
}

impl WeeklyAssessmentRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, assessment: &WeeklyAssessment) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO weekly_assessments (id, week_start, week_end, avg_score, total_focus_mins, total_productive_secs, total_distracting_secs, top_apps, summary)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
               ON CONFLICT(week_start) DO UPDATE SET
                   week_end = excluded.week_end,
                   avg_score = excluded.avg_score,
                   total_focus_mins = excluded.total_focus_mins,
                   total_productive_secs = excluded.total_productive_secs,
                   total_distracting_secs = excluded.total_distracting_secs,
                   top_apps = excluded.top_apps,
                   summary = excluded.summary"#,
        )
        .bind(&assessment.id)
        .bind(&assessment.week_start)
        .bind(&assessment.week_end)
        .bind(assessment.avg_score)
        .bind(assessment.total_focus_mins)
        .bind(assessment.total_productive_secs)
        .bind(assessment.total_distracting_secs)
        .bind(&assessment.top_apps)
        .bind(&assessment.summary)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_by_week(&self, week_start: &str) -> common::Result<Option<WeeklyAssessment>> {
        let row = sqlx::query_as::<_, WeeklyAssessment>(
            "SELECT id, week_start, week_end, avg_score, total_focus_mins, total_productive_secs, total_distracting_secs, top_apps, summary, created_at FROM weekly_assessments WHERE week_start = ?1",
        )
        .bind(week_start)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn list_recent(&self, limit: i64) -> common::Result<Vec<WeeklyAssessment>> {
        let rows = sqlx::query_as::<_, WeeklyAssessment>(
            "SELECT id, week_start, week_end, avg_score, total_focus_mins, total_productive_secs, total_distracting_secs, top_apps, summary, created_at FROM weekly_assessments ORDER BY week_start DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }
}
