use tracing::info;

use crate::repos::IntelligenceSessionRepo;
use crate::types::*;

/// Maximum FSRS stability for forecast confidence (same constant as cognitive::decay).
const MAX_STABILITY: f64 = 30.0;

pub struct PredictiveEngine {
    session_repo: IntelligenceSessionRepo,
}

impl PredictiveEngine {
    pub fn new(session_repo: IntelligenceSessionRepo) -> Self {
        Self { session_repo }
    }

    /// Generate energy forecasts for the next day based on the last 14 days of sessions.
    /// Produces hourly energy predictions grouped into time windows.
    pub async fn forecast_next_day(
        &self,
        date: jiff::civil::Date,
    ) -> common::Result<Vec<Forecast>> {
        let start = date
            .checked_sub(jiff::Span::new().days(14))
            .unwrap_or(date)
            .to_string()
            + "T00:00:00Z";
        let end = date.to_string() + "T23:59:59Z";

        let sessions = self.session_repo.list_range(&start, &end).await?;
        if sessions.is_empty() {
            return Ok(vec![]);
        }

        // Build hourly energy curve from historical session data.
        // Energy per hour = total focus duration in that hour / days that had data.
        let mut hourly_focus_secs = [0.0_f64; 24];
        let mut days_with_data = std::collections::HashSet::new();

        for session in &sessions {
            if session.session_type != "focus" {
                continue;
            }
            let Some(duration) = session.duration_secs else {
                continue;
            };
            // Parse hour from started_at
            if let Some(hour) = parse_hour(&session.started_at) {
                hourly_focus_secs[hour] += duration as f64;
                if let Some(day) = session.started_at.split('T').next() {
                    days_with_data.insert(day.to_string());
                }
            }
        }

        let num_days = days_with_data.len().max(1) as f64;
        let mut hourly_avg: Vec<f64> = hourly_focus_secs
            .iter()
            .map(|&s| s / num_days / 3600.0) // Normalize to 0.0-1.0 energy scale
            .collect();

        // Clamp energy values
        for v in &mut hourly_avg {
            *v = v.clamp(0.0, 1.0);
        }

        // Find peak and valley windows (top-2 peaks, bottom-2 valleys)
        let forecast_date = date
            .checked_add(jiff::Span::new().days(1))
            .unwrap_or(date)
            .to_string();
        let now = jiff::Timestamp::now().to_string();

        let mut forecasts = Vec::new();

        // Find peak energy windows (3-hour blocks)
        let windows = find_energy_windows(&hourly_avg);
        for (window_start_hour, window_end_hour, avg_energy) in windows.iter().take(3) {
            let confidence = (sessions.len() as f64 / 50.0).min(1.0); // More data = higher confidence
            let stability = (num_days / 7.0).min(1.0) * MAX_STABILITY * 0.5; // ~15 day stability with full week

            let forecast = Forecast {
                id: uuid::Uuid::new_v4().to_string(),
                forecast_date: forecast_date.clone(),
                forecast_type: ForecastType::Energy.to_string(),
                window_start: Some(format!("{:02}:00", window_start_hour)),
                window_end: Some(format!("{:02}:00", window_end_hour)),
                predicted_value: *avg_energy,
                confidence,
                stability,
                auto_protected: *avg_energy > 0.7,
                user_overrode: false,
                actual_value: None,
                prediction_error: None,
                created_at: now.clone(),
            };

            forecasts.push(forecast);
        }

        if !forecasts.is_empty() {
            info!(
                date = %forecast_date,
                count = forecasts.len(),
                "energy forecasts generated"
            );
        }

        Ok(forecasts)
    }

    /// Get the predicted energy level for the current time by interpolating forecasts.
    pub async fn current_energy(&self) -> common::Result<Option<f64>> {
        // Forecasts are no longer persisted; return None.
        Ok(None)
    }

    /// Evaluate forecast accuracy for a past date, update prediction_error and stability.
    pub async fn evaluate_accuracy(&self, _date: &str) -> common::Result<Option<AccuracyReport>> {
        // Forecasts are no longer persisted; return None.
        Ok(None)
    }

    /// Suggest protected blocks based on peak energy forecast windows.
    pub async fn suggest_protected_blocks(
        &self,
        _date: &str,
    ) -> common::Result<Vec<ProtectedBlock>> {
        // Forecasts are no longer persisted; return empty.
        Ok(vec![])
    }
}

/// FSRS-inspired stability update (same formula as cognitive::decay::update_stability).
/// Inlined to avoid L4→L5 dependency violation.
#[allow(dead_code)]
fn fsrs_update_stability(current: f64, success: bool, max: f64) -> f64 {
    if success {
        (current + (1.0 + current).ln().max(0.1)).min(max)
    } else {
        current
    }
}

/// Find the top energy windows (3-hour blocks) sorted by average energy descending.
fn find_energy_windows(hourly: &[f64]) -> Vec<(usize, usize, f64)> {
    let mut windows = Vec::new();
    // Consider hours 6–22 in 3-hour blocks
    for start in (6..20).step_by(3) {
        let end = (start + 3).min(24);
        let avg: f64 = hourly[start..end].iter().sum::<f64>() / (end - start) as f64;
        windows.push((start, end, avg));
    }
    windows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    windows
}

fn parse_hour(iso: &str) -> Option<usize> {
    // "2026-03-09T10:00:00Z" → 10
    let t_pos = iso.find('T')?;
    let time_part = &iso[t_pos + 1..];
    let hour: usize = time_part.get(..2)?.parse().ok()?;
    Some(hour)
}

#[allow(dead_code)]
fn parse_time_frac(time_str: &str) -> f64 {
    jiff::civil::Time::strptime("%H:%M", time_str)
        .map(|t| t.hour() as f64 + t.minute() as f64 / 60.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::IntelligenceSessionRepo;
    
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn make_session(id: &str, date: &str, hour: u32, duration_secs: i64) -> ProductivitySession {
        ProductivitySession {
            id: id.to_string(),
            session_type: "focus".to_string(),
            started_at: format!("{date}T{:02}:00:00Z", hour),
            ended_at: Some(format!(
                "{date}T{:02}:00:00Z",
                hour + (duration_secs / 3600) as u32
            )),
            duration_secs: Some(duration_secs),
            dominant_category: Some("coding".to_string()),
            category_purity: Some(0.9),
            quality_score: Some(0.8),
            source: SessionSource::AutoDetected.to_string(),
            app_breakdown: None,
            context_switches: 5,
            distraction_count: 3,
            predicted_energy: None,
            okr_alignment: None,
            notes: None,
            tags: None,
            action_id: None,
            project_id: None,
            target_mins: None,
            actual_mins: None,
            interruptions: None,
            distraction_events: None,
            completed: None,
            created_at: format!("{date}T{:02}:00:00Z", hour),
            updated_at: format!("{date}T{:02}:00:00Z", hour),
        }
    }

    async fn seed_history(pool: &SqlitePool) {
        let repo = IntelligenceSessionRepo::new(pool.clone());
        // Seed 7 days of sessions with morning peak (9-12) and afternoon peak (14-17)
        for day_offset in 0..7 {
            let date = jiff::civil::Date::new(2026, 3, 2 + day_offset as i8)
                .unwrap()
                .to_string();
            // Morning session
            let s1 = make_session(
                &format!("hist-m-{day_offset}"),
                &date,
                9,
                10800, // 3h
            );
            repo.create(&s1).await.unwrap();
            // Afternoon session
            let s2 = make_session(
                &format!("hist-a-{day_offset}"),
                &date,
                14,
                7200, // 2h
            );
            repo.create(&s2).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_forecast_next_day_uses_history() {
        let pool = setup_pool().await;
        seed_history(&pool).await;

        let engine = PredictiveEngine::new(IntelligenceSessionRepo::new(pool.clone()));

        let date = jiff::civil::Date::new(2026, 3, 9).unwrap();
        let forecasts = engine.forecast_next_day(date).await.unwrap();

        assert!(
            !forecasts.is_empty(),
            "should produce at least one forecast"
        );
        assert!(forecasts.len() <= 3, "should produce at most 3 forecasts");

        // The first forecast should be the highest energy window
        let first = &forecasts[0];
        assert!(first.predicted_value > 0.0);
        assert!(first.window_start.is_some());
        assert!(first.window_end.is_some());
    }

    #[tokio::test]
    async fn test_current_energy_returns_none_when_no_forecasts() {
        let pool = setup_pool().await;

        let engine = PredictiveEngine::new(IntelligenceSessionRepo::new(pool.clone()));

        // Forecasts are no longer persisted; current_energy returns None
        let energy = engine.current_energy().await.unwrap();
        assert!(energy.is_none());
    }

    #[tokio::test]
    async fn test_evaluate_accuracy_returns_none_when_no_forecasts() {
        let pool = setup_pool().await;

        let session_repo = IntelligenceSessionRepo::new(pool.clone());

        // Create a session for "2026-03-08"
        let session = make_session("acc-s1", "2026-03-08", 10, 7200);
        session_repo.create(&session).await.unwrap();

        let engine = PredictiveEngine::new(IntelligenceSessionRepo::new(pool.clone()));

        // Forecasts are no longer persisted; evaluate_accuracy returns None
        let report = engine.evaluate_accuracy("2026-03-08").await.unwrap();
        assert!(report.is_none());
    }

    #[tokio::test]
    async fn test_suggest_protected_blocks_returns_empty_when_no_forecasts() {
        let pool = setup_pool().await;

        let engine = PredictiveEngine::new(IntelligenceSessionRepo::new(pool.clone()));

        // Forecasts are no longer persisted; suggest_protected_blocks returns empty
        let blocks = engine.suggest_protected_blocks("2026-03-10").await.unwrap();
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_fsrs_update_stability() {
        let s = fsrs_update_stability(5.0, true, MAX_STABILITY);
        assert!(s > 5.0);
        assert!(s <= MAX_STABILITY);

        let s_fail = fsrs_update_stability(5.0, false, MAX_STABILITY);
        assert!((s_fail - 5.0).abs() < f64::EPSILON);
    }
}
