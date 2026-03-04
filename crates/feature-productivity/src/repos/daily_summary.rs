use sqlx::SqlitePool;

use crate::types::{AppUsage, CategoryUsage, DailySummary};

#[derive(sqlx::FromRow)]
struct SummaryRow {
    date: String,
    total_active_secs: i64,
    total_focus_secs: i64,
    total_break_secs: i64,
    total_idle_secs: i64,
    productive_secs: i64,
    neutral_secs: i64,
    distracting_secs: i64,
    focus_sessions_count: i64,
    avg_session_quality: Option<f64>,
    interruptions_count: i64,
    context_switches: i64,
    top_apps: Option<String>,
    top_categories: Option<String>,
    ai_summary: Option<String>,
}

impl From<SummaryRow> for DailySummary {
    fn from(row: SummaryRow) -> Self {
        let top_apps: Vec<AppUsage> = row
            .top_apps
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let top_categories: Vec<CategoryUsage> = row
            .top_categories
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        Self {
            date: row.date,
            total_active_secs: row.total_active_secs,
            total_focus_secs: row.total_focus_secs,
            total_break_secs: row.total_break_secs,
            total_idle_secs: row.total_idle_secs,
            productive_secs: row.productive_secs,
            neutral_secs: row.neutral_secs,
            distracting_secs: row.distracting_secs,
            focus_sessions_count: row.focus_sessions_count,
            avg_session_quality: row.avg_session_quality,
            interruptions_count: row.interruptions_count,
            context_switches: row.context_switches,
            top_apps,
            top_categories,
            ai_summary: row.ai_summary,
        }
    }
}

const SUMMARY_COLUMNS: &str = "date, total_active_secs, total_focus_secs, total_break_secs, total_idle_secs, productive_secs, neutral_secs, distracting_secs, focus_sessions_count, avg_session_quality, interruptions_count, context_switches, top_apps, top_categories, ai_summary";

#[derive(Debug, Clone)]
pub struct DailySummaryRepo {
    pool: SqlitePool,
}

impl DailySummaryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, summary: &DailySummary) -> common::Result<()> {
        let top_apps_json = serde_json::to_string(&summary.top_apps)
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let top_categories_json = serde_json::to_string(&summary.top_categories)
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        sqlx::query(
            r#"INSERT INTO daily_summaries (date, total_active_secs, total_focus_secs, total_break_secs, total_idle_secs, productive_secs, neutral_secs, distracting_secs, focus_sessions_count, avg_session_quality, interruptions_count, context_switches, top_apps, top_categories, ai_summary)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
               ON CONFLICT(date) DO UPDATE SET
                   total_active_secs = excluded.total_active_secs,
                   total_focus_secs = excluded.total_focus_secs,
                   total_break_secs = excluded.total_break_secs,
                   total_idle_secs = excluded.total_idle_secs,
                   productive_secs = excluded.productive_secs,
                   neutral_secs = excluded.neutral_secs,
                   distracting_secs = excluded.distracting_secs,
                   focus_sessions_count = excluded.focus_sessions_count,
                   avg_session_quality = excluded.avg_session_quality,
                   interruptions_count = excluded.interruptions_count,
                   context_switches = excluded.context_switches,
                   top_apps = excluded.top_apps,
                   top_categories = excluded.top_categories,
                   ai_summary = excluded.ai_summary,
                   computed_at = datetime('now')"#,
        )
        .bind(&summary.date)
        .bind(summary.total_active_secs)
        .bind(summary.total_focus_secs)
        .bind(summary.total_break_secs)
        .bind(summary.total_idle_secs)
        .bind(summary.productive_secs)
        .bind(summary.neutral_secs)
        .bind(summary.distracting_secs)
        .bind(summary.focus_sessions_count)
        .bind(summary.avg_session_quality)
        .bind(summary.interruptions_count)
        .bind(summary.context_switches)
        .bind(&top_apps_json)
        .bind(&top_categories_json)
        .bind(&summary.ai_summary)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get(&self, date: &str) -> common::Result<Option<DailySummary>> {
        let query = format!("SELECT {} FROM daily_summaries WHERE date = ?1", SUMMARY_COLUMNS);
        let row = sqlx::query_as::<_, SummaryRow>(&query)
            .bind(date)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(DailySummary::from))
    }

    pub async fn list_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Vec<DailySummary>> {
        let query = format!(
            "SELECT {} FROM daily_summaries WHERE date >= ?1 AND date <= ?2 ORDER BY date ASC",
            SUMMARY_COLUMNS
        );
        let rows = sqlx::query_as::<_, SummaryRow>(&query)
            .bind(start_date)
            .bind(end_date)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(DailySummary::from).collect())
    }
}
