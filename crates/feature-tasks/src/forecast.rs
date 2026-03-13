//! Pure computation for task estimation forecasting.
//!
//! No LLM calls, no I/O. Stateless functions for:
//! - Similarity matching between tasks
//! - Deviation correction (adjust estimates based on historical bias)
//! - Velocity calculation
//! - Accuracy statistics

use chrono::{DateTime, Utc};

/// A completed task's estimation record for similarity matching.
#[derive(Debug, Clone)]
pub struct EstimationRecord {
    pub task_id: String,
    pub tags: Vec<String>,
    pub energy_level: Option<String>,
    pub complexity_score: Option<i32>,
    pub project_id: Option<String>,
    pub estimated_minutes: i32,
    pub actual_minutes: i32,
    pub completed_at: DateTime<Utc>,
}

/// Compute similarity score between a target task and a historical record.
///
/// Weights from spec:
/// - Tags overlap (Jaccard): 0.35
/// - Energy level: 0.20
/// - Complexity score: 0.20
/// - Same project: 0.15
/// - Recency: 0.10
pub fn similarity(
    target_tags: &[String],
    target_energy: Option<&str>,
    target_complexity: Option<i32>,
    target_project: Option<&str>,
    record: &EstimationRecord,
    now: DateTime<Utc>,
) -> f64 {
    let tags_sim = jaccard_similarity(target_tags, &record.tags);
    let energy_sim = energy_similarity(target_energy, record.energy_level.as_deref());
    let complexity_sim = complexity_similarity(target_complexity, record.complexity_score);
    let project_sim = if target_project == record.project_id.as_deref() && target_project.is_some()
    {
        1.0
    } else {
        0.0
    };
    let recency = recency_score(record.completed_at, now);

    tags_sim * 0.35
        + energy_sim * 0.20
        + complexity_sim * 0.20
        + project_sim * 0.15
        + recency * 0.10
}

/// Jaccard similarity: |A ∩ B| / |A ∪ B|
fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let set_a: std::collections::HashSet<&str> = a.iter().map(|s| s.as_str()).collect();
    let set_b: std::collections::HashSet<&str> = b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Energy similarity: exact=1.0, adjacent=0.5, else 0.0
/// Order: low < medium < high < deep
fn energy_similarity(a: Option<&str>, b: Option<&str>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) if a == b => 1.0,
        (Some(a), Some(b)) => {
            let order = ["low", "medium", "high", "deep"];
            let pos_a = order.iter().position(|&x| x == a);
            let pos_b = order.iter().position(|&x| x == b);
            match (pos_a, pos_b) {
                (Some(pa), Some(pb)) if pa.abs_diff(pb) == 1 => 0.5,
                _ => 0.0,
            }
        }
        _ => 0.0,
    }
}

/// Complexity similarity: max(0.0, 1.0 - |a-b| / 10.0)
fn complexity_similarity(a: Option<i32>, b: Option<i32>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => (1.0 - (a - b).unsigned_abs() as f64 / 10.0).max(0.0),
        _ => 0.0,
    }
}

/// Recency score: e^(-days_ago / 30)
fn recency_score(completed_at: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let days_ago = (now - completed_at).num_days().max(0) as f64;
    (-days_ago / 30.0).exp()
}

/// Deviation correction result.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviationCorrection {
    pub adjusted_estimate: f64,
    pub optimistic: f64,
    pub pessimistic: f64,
    pub mean_deviation: f64,
    pub std_deviation: f64,
    pub sample_size: usize,
}

/// Apply deviation correction to an estimate using historical records.
///
/// Only considers records with similarity >= `threshold`.
/// Formula from spec:
///   adjusted = original × (1.0 + mean_deviation)
///   optimistic = original × (1.0 + mean_deviation - std_deviation)
///   pessimistic = original × (1.0 + mean_deviation + std_deviation)
pub fn deviation_correction(
    original_estimate: i32,
    records: &[(f64, &EstimationRecord)], // (similarity, record)
    threshold: f64,
) -> Option<DeviationCorrection> {
    let eligible: Vec<f64> = records
        .iter()
        .filter(|(sim, _)| *sim >= threshold)
        .filter(|(_, r)| r.estimated_minutes > 0)
        .map(|(_, r)| {
            (r.actual_minutes as f64 - r.estimated_minutes as f64) / r.estimated_minutes as f64
        })
        .collect();

    if eligible.is_empty() {
        return None;
    }

    let n = eligible.len();
    let mean = eligible.iter().sum::<f64>() / n as f64;
    let variance = eligible.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();
    let orig = original_estimate as f64;

    Some(DeviationCorrection {
        adjusted_estimate: orig * (1.0 + mean),
        optimistic: (orig * (1.0 + mean - std_dev)).max(1.0),
        pessimistic: orig * (1.0 + mean + std_dev),
        mean_deviation: mean,
        std_deviation: std_dev,
        sample_size: n,
    })
}

/// Data quality tier based on sample size.
#[derive(Debug, Clone, PartialEq)]
pub enum DataQualityTier {
    Strong,       // 20+
    Moderate,     // 10-19
    Weak,         // 5-9
    Insufficient, // <5
}

impl DataQualityTier {
    pub fn from_sample_size(n: usize) -> Self {
        match n {
            20.. => Self::Strong,
            10..=19 => Self::Moderate,
            5..=9 => Self::Weak,
            _ => Self::Insufficient,
        }
    }
}

/// Compute project velocity: completed minutes in last N weeks / N.
pub fn project_velocity(
    completed_records: &[EstimationRecord],
    now: DateTime<Utc>,
    weeks: u32,
) -> Option<f64> {
    let cutoff = now - chrono::Duration::weeks(weeks as i64);
    let recent: Vec<&EstimationRecord> = completed_records
        .iter()
        .filter(|r| r.completed_at >= cutoff)
        .collect();

    if recent.is_empty() {
        return None;
    }

    let total_mins: i32 = recent.iter().map(|r| r.actual_minutes).sum();
    Some(total_mins as f64 / weeks as f64)
}

/// Compute accuracy trend by comparing recent records (0-30 days) against older (31-90 days).
///
/// Returns `Insufficient` if either window has fewer than `min_sample` records.
/// Otherwise compares mean **absolute** deviation:
/// - `Improving` if recent |deviation| is >15% lower than older
/// - `Degrading` if recent |deviation| is >15% higher
/// - `Stable` otherwise
pub fn compute_trend(
    records: &[EstimationRecord],
    now: DateTime<Utc>,
    min_sample: usize,
) -> crate::types::AccuracyTrend {
    use crate::types::AccuracyTrend;

    let cutoff_30 = now - chrono::Duration::days(30);
    let cutoff_90 = now - chrono::Duration::days(90);

    let (mut recent, mut older) = (Vec::new(), Vec::new());

    for r in records {
        if r.estimated_minutes == 0 {
            continue;
        }
        let dev = ((r.actual_minutes as f64 - r.estimated_minutes as f64)
            / r.estimated_minutes as f64)
            .abs();
        if r.completed_at >= cutoff_30 {
            recent.push(dev);
        } else if r.completed_at >= cutoff_90 {
            older.push(dev);
        }
    }

    if recent.len() < min_sample || older.len() < min_sample {
        return AccuracyTrend::Insufficient;
    }

    let mean_recent = recent.iter().sum::<f64>() / recent.len() as f64;
    let mean_older = older.iter().sum::<f64>() / older.len() as f64;

    if mean_older < 0.01 {
        // Older deviation near zero — can't meaningfully compare
        return AccuracyTrend::Stable;
    }

    let change_ratio = (mean_recent - mean_older) / mean_older;

    if change_ratio < -0.15 {
        AccuracyTrend::Improving
    } else if change_ratio > 0.15 {
        AccuracyTrend::Degrading
    } else {
        AccuracyTrend::Stable
    }
}

/// Accuracy statistics for a set of estimation records.
#[derive(Debug, Clone)]
pub struct AccuracyStats {
    pub count: usize,
    pub mean_deviation_pct: f64,
    pub median_deviation_pct: f64,
    pub p90_deviation_pct: f64,
    pub std_deviation_pct: f64,
}

/// Compute accuracy statistics from estimation records.
pub fn accuracy_stats(records: &[EstimationRecord]) -> Option<AccuracyStats> {
    if records.is_empty() {
        return None;
    }

    let mut deviations: Vec<f64> = records
        .iter()
        .filter(|r| r.estimated_minutes > 0)
        .map(|r| {
            ((r.actual_minutes as f64 - r.estimated_minutes as f64) / r.estimated_minutes as f64)
                * 100.0
        })
        .collect();

    if deviations.is_empty() {
        return None;
    }
    deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = deviations.len();
    let mean = deviations.iter().sum::<f64>() / n as f64;
    let variance = deviations.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n as f64;
    let median = if n.is_multiple_of(2) {
        (deviations[n / 2 - 1] + deviations[n / 2]) / 2.0
    } else {
        deviations[n / 2]
    };
    let p90_idx = ((n as f64 * 0.9).ceil() as usize).min(n) - 1;

    Some(AccuracyStats {
        count: n,
        mean_deviation_pct: mean,
        median_deviation_pct: median,
        p90_deviation_pct: deviations[p90_idx],
        std_deviation_pct: variance.sqrt(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn record(
        tags: &[&str],
        energy: &str,
        complexity: i32,
        project: &str,
        est: i32,
        actual: i32,
        days_ago: i64,
    ) -> EstimationRecord {
        EstimationRecord {
            task_id: uuid::Uuid::new_v4().to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            energy_level: Some(energy.into()),
            complexity_score: Some(complexity),
            project_id: Some(project.into()),
            estimated_minutes: est,
            actual_minutes: actual,
            completed_at: Utc::now() - Duration::days(days_ago),
        }
    }

    #[test]
    fn test_jaccard_identical() {
        let a = vec!["rust".into(), "backend".into()];
        assert!((jaccard_similarity(&a, &a) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_disjoint() {
        let a = vec!["rust".into()];
        let b = vec!["python".into()];
        assert!((jaccard_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_partial() {
        let a = vec!["rust".into(), "backend".into()];
        let b = vec!["rust".into(), "frontend".into()];
        assert!((jaccard_similarity(&a, &b) - 1.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_energy_exact_match() {
        assert!((energy_similarity(Some("high"), Some("high")) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_energy_adjacent() {
        assert!((energy_similarity(Some("high"), Some("deep")) - 0.5).abs() < 0.001);
        assert!((energy_similarity(Some("low"), Some("medium")) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_energy_distant() {
        assert!((energy_similarity(Some("low"), Some("deep")) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_complexity_same() {
        assert!((complexity_similarity(Some(5), Some(5)) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_complexity_far() {
        assert!((complexity_similarity(Some(1), Some(10)) - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_deviation_correction_underestimate() {
        let records = [
            record(&["rust"], "high", 5, "p1", 30, 60, 5), // +100% deviation
            record(&["rust"], "high", 5, "p1", 45, 60, 10), // +33% deviation
        ];
        let scored: Vec<(f64, &EstimationRecord)> = records.iter().map(|r| (0.8, r)).collect();
        let result = deviation_correction(30, &scored, 0.3).unwrap();

        // mean deviation = (1.0 + 0.333) / 2 = 0.667
        assert!(result.adjusted_estimate > 30.0);
        assert!(result.pessimistic > result.adjusted_estimate);
        assert!(result.optimistic < result.adjusted_estimate);
        assert_eq!(result.sample_size, 2);
    }

    #[test]
    fn test_deviation_correction_threshold_filters() {
        let records = [record(&["rust"], "high", 5, "p1", 30, 60, 5)];
        let scored: Vec<(f64, &EstimationRecord)> = records.iter().map(|r| (0.1, r)).collect();
        let result = deviation_correction(30, &scored, 0.3);
        assert!(result.is_none(), "Below threshold should return None");
    }

    #[test]
    fn test_data_quality_tiers() {
        assert_eq!(
            DataQualityTier::from_sample_size(25),
            DataQualityTier::Strong
        );
        assert_eq!(
            DataQualityTier::from_sample_size(15),
            DataQualityTier::Moderate
        );
        assert_eq!(DataQualityTier::from_sample_size(7), DataQualityTier::Weak);
        assert_eq!(
            DataQualityTier::from_sample_size(3),
            DataQualityTier::Insufficient
        );
    }

    #[test]
    fn test_project_velocity() {
        let now = Utc::now();
        let records = vec![
            record(&[], "medium", 3, "p1", 30, 45, 3),
            record(&[], "medium", 3, "p1", 60, 55, 10),
            record(&[], "medium", 3, "p1", 90, 100, 50), // > 4 weeks ago
        ];
        let v = project_velocity(&records, now, 4).unwrap();
        // 45 + 55 = 100 mins in 4 weeks = 25 mins/week
        assert!((v - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_project_velocity_none_when_empty() {
        assert!(project_velocity(&[], Utc::now(), 4).is_none());
    }

    #[test]
    fn test_accuracy_stats_basic() {
        let records = vec![
            record(&[], "medium", 3, "p1", 30, 45, 5),  // +50%
            record(&[], "medium", 3, "p1", 60, 90, 10), // +50%
            record(&[], "medium", 3, "p1", 40, 40, 15), // 0%
        ];
        let stats = accuracy_stats(&records).unwrap();
        assert_eq!(stats.count, 3);
        // mean: (50 + 50 + 0) / 3 = 33.3%
        assert!((stats.mean_deviation_pct - 33.33).abs() < 0.1);
    }

    #[test]
    fn test_compute_trend_improving() {
        use crate::types::AccuracyTrend;
        let now = Utc::now();
        let mut records = Vec::new();
        // Recent (0-30 days): estimates are close to actuals (low deviation)
        for i in 0..6 {
            records.push(record(&[], "medium", 3, "p1", 60, 63, i + 1)); // ~5% deviation
        }
        // Older (31-90 days): estimates were way off (high deviation)
        for i in 0..6 {
            records.push(record(&[], "medium", 3, "p1", 60, 96, 35 + i)); // ~60% deviation
        }
        assert_eq!(compute_trend(&records, now, 5), AccuracyTrend::Improving);
    }

    #[test]
    fn test_compute_trend_degrading() {
        use crate::types::AccuracyTrend;
        let now = Utc::now();
        let mut records = Vec::new();
        // Recent: estimates are way off
        for i in 0..6 {
            records.push(record(&[], "medium", 3, "p1", 60, 96, i + 1)); // ~60% deviation
        }
        // Older: estimates were accurate
        for i in 0..6 {
            records.push(record(&[], "medium", 3, "p1", 60, 63, 35 + i)); // ~5% deviation
        }
        assert_eq!(compute_trend(&records, now, 5), AccuracyTrend::Degrading);
    }

    #[test]
    fn test_compute_trend_stable() {
        use crate::types::AccuracyTrend;
        let now = Utc::now();
        let mut records = Vec::new();
        // Both windows: similar deviation (~25-27%)
        for i in 0..6 {
            records.push(record(&[], "medium", 3, "p1", 60, 76, i + 1)); // ~26.7%
        }
        for i in 0..6 {
            records.push(record(&[], "medium", 3, "p1", 60, 75, 35 + i)); // ~25%
        }
        assert_eq!(compute_trend(&records, now, 5), AccuracyTrend::Stable);
    }

    #[test]
    fn test_compute_trend_insufficient_recent() {
        use crate::types::AccuracyTrend;
        let now = Utc::now();
        let mut records = Vec::new();
        // Only 2 recent records (below min_sample=5)
        for i in 0..2 {
            records.push(record(&[], "medium", 3, "p1", 60, 63, i + 1));
        }
        for i in 0..6 {
            records.push(record(&[], "medium", 3, "p1", 60, 96, 35 + i));
        }
        assert_eq!(compute_trend(&records, now, 5), AccuracyTrend::Insufficient);
    }
}
