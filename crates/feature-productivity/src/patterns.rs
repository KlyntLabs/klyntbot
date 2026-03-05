//! Productivity pattern analyzer — detects usage patterns from focus sessions
//! and daily summaries for personalized agent context.

use chrono::{DateTime, Datelike, Duration, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};

use crate::repos::ProductivityRepos;
use crate::types::{DailySummary, FocusSession};

/// Detected productivity patterns, serialized to `learning_state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductivityPatterns {
    /// Hours of the day with highest focus session quality (0-23).
    pub peak_focus_hours: Vec<u32>,
    /// Trending average focus session duration in minutes.
    pub avg_session_mins: f64,
    /// Ratio of productive time to total active time (0.0-1.0).
    pub productive_ratio: f64,
    /// Average daily context switches.
    pub avg_context_switches: f64,
    /// Most productive day of week, if enough data.
    pub best_day_of_week: Option<Weekday>,
    /// Number of days analyzed.
    pub days_analyzed: usize,
    /// When this analysis was computed.
    pub computed_at: DateTime<Utc>,
}

pub struct ProductivityPatternAnalyzer {
    repos: ProductivityRepos,
}

impl ProductivityPatternAnalyzer {
    pub fn new(repos: ProductivityRepos) -> Self {
        Self { repos }
    }

    /// Analyze the last `days` of data and return detected patterns.
    pub async fn analyze(&self, days: u32) -> common::Result<ProductivityPatterns> {
        let now = Utc::now();
        let start = now - Duration::days(i64::from(days));
        let start_date = start.format("%Y-%m-%d").to_string();
        let end_date = now.format("%Y-%m-%d").to_string();

        let (summaries, sessions) = tokio::try_join!(
            self.repos.summaries.list_range(&start_date, &end_date),
            self.repos.sessions.list_range(&start, &now, None),
        )?;

        let peak_focus_hours = Self::compute_peak_hours(&sessions);
        let avg_session_mins = Self::compute_avg_session_mins(&sessions);
        let productive_ratio = Self::compute_productive_ratio(&summaries);
        let avg_context_switches = Self::compute_avg_switches(&summaries);
        let best_day_of_week = Self::compute_best_day(&summaries);

        Ok(ProductivityPatterns {
            peak_focus_hours,
            avg_session_mins,
            productive_ratio,
            avg_context_switches,
            best_day_of_week,
            days_analyzed: summaries.len(),
            computed_at: Utc::now(),
        })
    }

    /// Find hours where completed sessions have the highest average quality.
    fn compute_peak_hours(sessions: &[FocusSession]) -> Vec<u32> {
        let completed: Vec<_> = sessions
            .iter()
            .filter(|s| s.completed && s.quality_score.is_some())
            .collect();

        if completed.is_empty() {
            return vec![];
        }

        // Group quality scores by hour
        let mut hour_scores: [Vec<f64>; 24] = std::array::from_fn(|_| Vec::new());
        for s in &completed {
            let hour = s.started_at.hour() as usize;
            if let Some(q) = s.quality_score {
                hour_scores[hour].push(q);
            }
        }

        // Compute average per hour, keep hours with >= 2 sessions
        let mut hour_avgs: Vec<(u32, f64)> = hour_scores
            .iter()
            .enumerate()
            .filter(|(_, scores)| scores.len() >= 2)
            .map(|(h, scores)| {
                let avg = scores.iter().sum::<f64>() / scores.len() as f64;
                (h as u32, avg)
            })
            .collect();

        // Sort by quality descending, take top 3
        hour_avgs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hour_avgs.into_iter().take(3).map(|(h, _)| h).collect()
    }

    fn compute_avg_session_mins(sessions: &[FocusSession]) -> f64 {
        let (sum, count) = sessions
            .iter()
            .filter_map(|s| if s.completed { s.actual_mins } else { None })
            .fold((0i64, 0usize), |(sum, n), mins| (sum + mins, n + 1));

        if count == 0 {
            return 0.0;
        }

        sum as f64 / count as f64
    }

    fn compute_productive_ratio(summaries: &[DailySummary]) -> f64 {
        let total_active: i64 = summaries.iter().map(|s| s.total_active_secs).sum();
        let total_productive: i64 = summaries.iter().map(|s| s.productive_secs).sum();

        if total_active == 0 {
            return 0.0;
        }

        total_productive as f64 / total_active as f64
    }

    fn compute_avg_switches(summaries: &[DailySummary]) -> f64 {
        if summaries.is_empty() {
            return 0.0;
        }

        let total: i64 = summaries.iter().map(|s| s.context_switches).sum();
        total as f64 / summaries.len() as f64
    }

    /// Find the day of week with the highest average productive_secs.
    fn compute_best_day(summaries: &[DailySummary]) -> Option<Weekday> {
        if summaries.len() < 7 {
            return None;
        }

        // Group by weekday (Mon=0, Sun=6)
        let mut day_totals: [(f64, usize); 7] = [(0.0, 0); 7];
        for s in summaries {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&s.date, "%Y-%m-%d") {
                let weekday = date.weekday().num_days_from_monday() as usize;
                day_totals[weekday].0 += s.productive_secs as f64;
                day_totals[weekday].1 += 1;
            }
        }

        use chrono::Weekday::*;
        const WEEKDAYS: [Weekday; 7] = [Mon, Tue, Wed, Thu, Fri, Sat, Sun];

        day_totals
            .iter()
            .enumerate()
            .filter(|(_, (_, count))| *count > 0)
            .max_by(|(_, (a_total, a_count)), (_, (b_total, b_count))| {
                let a_avg = a_total / *a_count as f64;
                let b_avg = b_total / *b_count as f64;
                a_avg
                    .partial_cmp(&b_avg)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(day, _)| WEEKDAYS[day])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SessionType;

    #[test]
    fn test_peak_hours_empty() {
        let hours = ProductivityPatternAnalyzer::compute_peak_hours(&[]);
        assert!(hours.is_empty());
    }

    #[test]
    fn test_peak_hours_with_data() {
        let now = Utc::now();
        let sessions: Vec<FocusSession> = (0..6)
            .map(|i| {
                let hour_offset = if i < 3 { 9 } else { 14 }; // 3 at 9am, 3 at 2pm
                let quality = if i < 3 { 0.9 } else { 0.6 };
                FocusSession {
                    id: format!("s{}", i),
                    action_id: None,
                    project_id: None,
                    session_type: SessionType::Focus,
                    target_mins: Some(25),
                    started_at: now
                        .date_naive()
                        .and_hms_opt(hour_offset, 0, 0)
                        .unwrap()
                        .and_utc(),
                    ended_at: Some(
                        now.date_naive()
                            .and_hms_opt(hour_offset, 25, 0)
                            .unwrap()
                            .and_utc(),
                    ),
                    actual_mins: Some(25),
                    interruptions: 0,
                    distraction_events: vec![],
                    quality_score: Some(quality),
                    completed: true,
                    notes: None,
                }
            })
            .collect();

        let hours = ProductivityPatternAnalyzer::compute_peak_hours(&sessions);
        assert!(!hours.is_empty());
        // 9am should be first (higher quality)
        assert_eq!(hours[0], 9);
    }

    #[test]
    fn test_avg_session_mins() {
        let now = Utc::now();
        let sessions = vec![
            FocusSession {
                id: "s1".to_string(),
                action_id: None,
                project_id: None,
                session_type: SessionType::Focus,
                target_mins: Some(25),
                started_at: now,
                ended_at: Some(now + Duration::minutes(25)),
                actual_mins: Some(25),
                interruptions: 0,
                distraction_events: vec![],
                quality_score: Some(0.8),
                completed: true,
                notes: None,
            },
            FocusSession {
                id: "s2".to_string(),
                actual_mins: Some(45),
                completed: true,
                ..sessions_base(now)
            },
        ];

        let avg = ProductivityPatternAnalyzer::compute_avg_session_mins(&sessions);
        assert!((avg - 35.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_productive_ratio() {
        let summaries = vec![
            DailySummary {
                date: "2026-03-01".to_string(),
                total_active_secs: 3600,
                productive_secs: 2700,
                ..summary_base()
            },
            DailySummary {
                date: "2026-03-02".to_string(),
                total_active_secs: 3600,
                productive_secs: 1800,
                ..summary_base()
            },
        ];

        let ratio = ProductivityPatternAnalyzer::compute_productive_ratio(&summaries);
        assert!((ratio - 0.625).abs() < 0.001); // 4500/7200
    }

    fn sessions_base(now: DateTime<Utc>) -> FocusSession {
        FocusSession {
            id: String::new(),
            action_id: None,
            project_id: None,
            session_type: SessionType::Focus,
            target_mins: Some(25),
            started_at: now,
            ended_at: None,
            actual_mins: None,
            interruptions: 0,
            distraction_events: vec![],
            quality_score: None,
            completed: false,
            notes: None,
        }
    }

    fn summary_base() -> DailySummary {
        DailySummary {
            date: String::new(),
            total_active_secs: 0,
            total_focus_secs: 0,
            total_break_secs: 0,
            total_idle_secs: 0,
            productive_secs: 0,
            neutral_secs: 0,
            distracting_secs: 0,
            focus_sessions_count: 0,
            avg_session_quality: None,
            interruptions_count: 0,
            context_switches: 0,
            top_apps: vec![],
            top_categories: vec![],
            productivity_score: None,
            ai_summary: None,
        }
    }
}
