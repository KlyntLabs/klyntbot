//! LLM-enhanced forecast handler.
//!
//! Wraps feature_tasks::forecast (pure computation) with LLM-powered
//! risk narrative generation. The L4 module handles similarity matching,
//! deviation correction, and accuracy stats. This L5 handler adds
//! contextual risk analysis.

use std::collections::HashMap;

use async_trait::async_trait;
use common::Result;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::debug;

use feature_tasks::forecast::{self as fc, DataQualityTier};
use feature_tasks::types::{
    AccuracyReport, AccuracyScope, AccuracyTrend, DataQuality, ForecastContext,
    ForecastMethodology, ForecastRisk, ProjectForecast, RiskKind, Task, TaskForecast,
};
use feature_tasks::ForecastHandler;
use storage::TaskRepo;

static RISK_PROMPT: &str = include_str!("../templates/forecast_risk.md");

pub struct LlmForecastHandler {
    provider: DynProvider,
    model: String,
    repo: TaskRepo,
}

impl LlmForecastHandler {
    pub fn new(provider: DynProvider, model: String, repo: TaskRepo) -> Self {
        Self {
            provider,
            model,
            repo,
        }
    }

    /// Load estimation records from storage as L4 forecast records.
    async fn load_records(&self, lookback_days: u32) -> Result<Vec<fc::EstimationRecord>> {
        let rows = self.repo.list_estimation_history(lookback_days).await?;
        Ok(rows
            .into_iter()
            .map(|r| fc::EstimationRecord {
                task_id: r.task_id,
                tags: r.tags,
                energy_level: r.energy_level,
                complexity_score: r.complexity_score,
                // project_id not stored in estimation rows; similarity scoring
                // will skip the project bonus (0.15 weight) for now.
                project_id: None,
                estimated_minutes: r.estimated_minutes,
                actual_minutes: r.actual_minutes,
                completed_at: r.completed_at,
            })
            .collect())
    }
}

fn to_data_quality(tier: &DataQualityTier) -> DataQuality {
    match tier {
        DataQualityTier::Strong => DataQuality::High,
        DataQualityTier::Moderate => DataQuality::Medium,
        DataQualityTier::Weak => DataQuality::Low,
        DataQualityTier::Insufficient => DataQuality::Insufficient,
    }
}

/// Input data for building the risk analysis prompt.
struct RiskPromptData<'a> {
    subject: &'a str,
    sample_size: usize,
    data_quality: &'a str,
    mean_deviation: f64,
    estimated_minutes: i32,
    confidence_low: i32,
    confidence_high: i32,
    context: &'a str,
}

/// Build the risk analysis prompt from forecast data.
fn build_risk_prompt(data: &RiskPromptData<'_>) -> String {
    RISK_PROMPT
        .replace("{{ subject }}", data.subject)
        .replace("{{ sample_size }}", &data.sample_size.to_string())
        .replace("{{ data_quality }}", data.data_quality)
        .replace(
            "{{ mean_deviation }}",
            &format!("{:.1}", data.mean_deviation),
        )
        .replace(
            "{{ estimated_minutes }}",
            &data.estimated_minutes.to_string(),
        )
        .replace("{{ confidence_low }}", &data.confidence_low.to_string())
        .replace("{{ confidence_high }}", &data.confidence_high.to_string())
        .replace("{{ context }}", data.context)
}

/// Parse LLM risk analysis response into ForecastRisk structs.
fn parse_risks(response: &str) -> Vec<ForecastRisk> {
    let json_str = common::helpers::strip_llm_fences(response);

    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    parsed
        .get("risks")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let kind: RiskKind = serde_json::from_value(serde_json::Value::String(
                        r.get("kind")?.as_str()?.into(),
                    ))
                    .ok()?;
                    Some(ForecastRisk {
                        kind,
                        description: r.get("description")?.as_str()?.to_string(),
                        impact_minutes: r
                            .get("impact_minutes")
                            .and_then(|m| m.as_i64())
                            .map(|m| m as i32),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl ForecastHandler for LlmForecastHandler {
    async fn forecast_task(&self, task: &Task, context: &ForecastContext) -> Result<TaskForecast> {
        let records = self.load_records(context.lookback_days).await?;
        let now = chrono::Utc::now();

        // Score each record for similarity against target task
        let scored: Vec<(f64, &fc::EstimationRecord)> = records
            .iter()
            .map(|r| {
                let sim = fc::similarity(
                    &task.tags,
                    task.energy_level.as_ref().map(|e| e.to_string()).as_deref(),
                    task.complexity_score,
                    task.project_id.as_deref(),
                    r,
                    now,
                );
                (sim, r)
            })
            .collect();

        let min_sample = context.min_sample_size as usize;
        let threshold = if scored.iter().filter(|(s, _)| *s >= 0.3).count() >= min_sample {
            0.3
        } else {
            0.1
        };

        let original_est = task.estimated_minutes.unwrap_or(60);
        let correction = fc::deviation_correction(original_est, &scored, threshold);
        let sample_size = correction.as_ref().map_or(0, |c| c.sample_size);
        let quality_tier = DataQualityTier::from_sample_size(sample_size);

        let (adjusted, optimistic, pessimistic, mean_dev) = match &correction {
            Some(c) => (
                c.adjusted_estimate as i32,
                c.optimistic as i32,
                c.pessimistic as i32,
                c.mean_deviation,
            ),
            None => (original_est, original_est, original_est, 0.0),
        };

        // LLM risk analysis (non-fatal on failure)
        let risks = match self
            .provider
            .chat(
                &[
                    Message::system(
                        "You are a task estimation risk analyst. Return only valid JSON.",
                    ),
                    Message::user(build_risk_prompt(&RiskPromptData {
                        subject: &task.title,
                        sample_size,
                        data_quality: &format!("{quality_tier:?}"),
                        mean_deviation: mean_dev * 100.0,
                        estimated_minutes: adjusted,
                        confidence_low: optimistic,
                        confidence_high: pessimistic,
                        context: "",
                    })),
                ],
                None,
                &ChatParams::new(&self.model)
                    .with_temperature(0.2)
                    .with_max_tokens(2048)
                    .with_response_format(ResponseFormat::JsonObject),
            )
            .await
        {
            Ok(r) => parse_risks(&r.content.unwrap_or_default()),
            Err(e) => {
                debug!("Risk analysis LLM call failed (non-fatal): {e}");
                vec![]
            }
        };

        let adjustments = if correction.is_some() {
            vec![format!("deviation correction ({:+.0}%)", mean_dev * 100.0)]
        } else {
            vec![]
        };

        Ok(TaskForecast {
            task_id: task.id.clone(),
            estimated_minutes: adjusted,
            confidence_low: optimistic,
            confidence_high: pessimistic,
            methodology: ForecastMethodology {
                name: "historical_similarity".into(),
                sample_size: sample_size as u32,
                lookback_days: context.lookback_days,
                adjustments,
            },
            risks,
            data_quality: to_data_quality(&quality_tier),
        })
    }

    async fn forecast_project(
        &self,
        project_id: &str,
        context: &ForecastContext,
    ) -> Result<ProjectForecast> {
        let records = self.load_records(context.lookback_days).await?;

        // Load incomplete tasks for project
        let filter = storage::TaskFilter {
            project_id: Some(project_id.into()),
            status: Some("todo".into()),
            ..Default::default()
        };
        let remaining_tasks = self.repo.list(&filter).await?;
        let remaining_mins: i32 = remaining_tasks
            .iter()
            .map(|t| t.estimated_minutes.unwrap_or(60))
            .sum();

        // Count completed tasks for this project via task list (not estimation rows,
        // which lack project_id). This gives an accurate per-project count.
        let completed_filter = storage::TaskFilter {
            project_id: Some(project_id.into()),
            status: Some("done".into()),
            ..Default::default()
        };
        let completed_count = self.repo.list(&completed_filter).await?.len();

        let quality_tier = DataQualityTier::from_sample_size(completed_count);

        // Confidence interval: ±30%/50% of remaining estimate.
        // Velocity is checked only to confirm we have recent data.
        let has_velocity =
            fc::project_velocity(&records, chrono::Utc::now(), 4).is_some_and(|v| v > 0.0);
        let (confidence_low, confidence_high) = if has_velocity {
            (
                (remaining_mins as f64 * 0.7) as i32,
                (remaining_mins as f64 * 1.5) as i32,
            )
        } else {
            (remaining_mins, (remaining_mins as f64 * 1.5) as i32)
        };

        Ok(ProjectForecast {
            project_id: project_id.to_string(),
            total_estimated_minutes: remaining_mins,
            confidence_low,
            confidence_high,
            remaining_tasks: remaining_tasks.len() as u32,
            completed_tasks: completed_count as u32,
            risks: vec![],
            data_quality: to_data_quality(&quality_tier),
        })
    }

    async fn accuracy_stats(&self, scope: &AccuracyScope) -> Result<AccuracyReport> {
        let lookback = scope.lookback_days.unwrap_or(90);
        let records = self.load_records(lookback).await?;

        let stats = fc::accuracy_stats(&records);

        // Compute per-energy-level breakdowns
        let by_energy_level = compute_breakdown(&records, |r| r.energy_level.clone());
        let by_complexity =
            compute_breakdown(&records, |r| r.complexity_score.map(|c| c.to_string()));

        Ok(match stats {
            Some(s) => AccuracyReport {
                scope: scope.clone(),
                sample_size: s.count as u32,
                mean_deviation_pct: s.mean_deviation_pct,
                median_deviation_pct: s.median_deviation_pct,
                p90_deviation_pct: s.p90_deviation_pct,
                trend: AccuracyTrend::Insufficient, // TODO: compute trend from recent vs older
                by_energy_level,
                by_complexity,
            },
            None => AccuracyReport {
                scope: scope.clone(),
                sample_size: 0,
                mean_deviation_pct: 0.0,
                median_deviation_pct: 0.0,
                p90_deviation_pct: 0.0,
                trend: AccuracyTrend::Insufficient,
                by_energy_level: HashMap::new(),
                by_complexity: HashMap::new(),
            },
        })
    }
}

/// Compute mean deviation grouped by an arbitrary key extracted from each record.
fn compute_breakdown(
    records: &[fc::EstimationRecord],
    key_fn: impl Fn(&fc::EstimationRecord) -> Option<String>,
) -> HashMap<String, f64> {
    let mut groups: HashMap<String, Vec<f64>> = HashMap::new();
    for r in records {
        if r.estimated_minutes == 0 {
            continue;
        }
        if let Some(key) = key_fn(r) {
            let dev = (r.actual_minutes as f64 - r.estimated_minutes as f64)
                / r.estimated_minutes as f64
                * 100.0;
            groups.entry(key).or_default().push(dev);
        }
    }
    groups
        .into_iter()
        .map(|(k, v)| {
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            (k, mean)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_risks_valid() {
        let json = r#"{"risks": [{"kind": "historicalunderestimation", "description": "You underestimate", "impact_minutes": 30}]}"#;
        let risks = parse_risks(json);
        assert_eq!(risks.len(), 1);
        assert_eq!(risks[0].kind, RiskKind::HistoricalUnderestimation);
        assert_eq!(risks[0].description, "You underestimate");
        assert_eq!(risks[0].impact_minutes, Some(30));
    }

    #[test]
    fn test_parse_risks_empty() {
        let json = r#"{"risks": []}"#;
        let risks = parse_risks(json);
        assert!(risks.is_empty());
    }

    #[test]
    fn test_parse_risks_invalid() {
        let risks = parse_risks("not json");
        assert!(risks.is_empty());
    }

    #[test]
    fn test_parse_risks_with_fences() {
        let json = "```json\n{\"risks\": [{\"kind\": \"unknowncomplexity\", \"description\": \"Complex task\", \"impact_minutes\": null}]}\n```";
        let risks = parse_risks(json);
        assert_eq!(risks.len(), 1);
        assert_eq!(risks[0].kind, RiskKind::UnknownComplexity);
        assert_eq!(risks[0].impact_minutes, None);
    }

    #[test]
    fn test_build_risk_prompt_substitutions() {
        let prompt = build_risk_prompt(&RiskPromptData {
            subject: "Fix login bug",
            sample_size: 15,
            data_quality: "High",
            mean_deviation: 38.0,
            estimated_minutes: 45,
            confidence_low: 30,
            confidence_high: 60,
            context: "overdue",
        });
        assert!(prompt.contains("Fix login bug"));
        assert!(prompt.contains("15"));
        assert!(prompt.contains("High"));
        assert!(prompt.contains("38.0"));
        assert!(prompt.contains("45"));
        assert!(prompt.contains("30"));
        assert!(prompt.contains("60"));
        assert!(prompt.contains("overdue"));
    }

    #[test]
    fn test_to_data_quality() {
        assert_eq!(to_data_quality(&DataQualityTier::Strong), DataQuality::High);
        assert_eq!(
            to_data_quality(&DataQualityTier::Moderate),
            DataQuality::Medium
        );
        assert_eq!(to_data_quality(&DataQualityTier::Weak), DataQuality::Low);
        assert_eq!(
            to_data_quality(&DataQualityTier::Insufficient),
            DataQuality::Insufficient
        );
    }

    #[test]
    fn test_compute_breakdown_by_energy() {
        let records = vec![
            fc::EstimationRecord {
                task_id: "t1".into(),
                tags: vec![],
                energy_level: Some("high".into()),
                complexity_score: None,
                project_id: None,
                estimated_minutes: 30,
                actual_minutes: 45,
                completed_at: chrono::Utc::now(),
            },
            fc::EstimationRecord {
                task_id: "t2".into(),
                tags: vec![],
                energy_level: Some("high".into()),
                complexity_score: None,
                project_id: None,
                estimated_minutes: 60,
                actual_minutes: 90,
                completed_at: chrono::Utc::now(),
            },
        ];
        let breakdown = compute_breakdown(&records, |r| r.energy_level.clone());
        assert_eq!(breakdown.len(), 1);
        // Both are +50% deviation
        assert!((breakdown["high"] - 50.0).abs() < 0.1);
    }
}
