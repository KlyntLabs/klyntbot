//! Cost and usage tracker — records per-request LLM usage to JSONL.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use common::Result;
use providers::Usage;

/// A single usage record written to usage.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
    pub model: String,
    pub provider: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
    pub estimated_cost_usd: f64,
    pub channel: String,
    pub strategy: String,
}

/// Aggregated usage report.
#[derive(Debug)]
pub struct UsageReport {
    pub total_requests: usize,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    /// model → (tokens, cost)
    pub by_model: HashMap<String, (u64, f64)>,
    /// date string → cost
    pub by_day: Vec<(String, f64)>,
}

/// Tracks LLM usage and costs, persisted to JSONL or SQL.
pub struct CostTracker {
    data_dir: PathBuf,
    sql_repo: Option<storage::UsageRepo>,
}

/// Per-million-token pricing: (input_per_mtok, output_per_mtok).
fn model_pricing(model: &str) -> (f64, f64) {
    let m = model.to_lowercase();
    if m.contains("opus") {
        (15.0, 75.0)
    } else if m.contains("sonnet") {
        (3.0, 15.0)
    } else if m.contains("haiku") {
        (0.25, 1.25)
    } else if m.contains("gpt-4o") {
        (2.50, 10.0)
    } else {
        (0.0, 0.0) // unknown model, don't crash
    }
}

fn estimate_cost(usage: &Usage, model: &str) -> f64 {
    let (input_rate, output_rate) = model_pricing(model);
    let input_cost = (usage.prompt_tokens as f64 / 1_000_000.0) * input_rate;
    let output_cost = (usage.completion_tokens as f64 / 1_000_000.0) * output_rate;
    input_cost + output_cost
}

impl CostTracker {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            sql_repo: None,
        }
    }

    /// Create a CostTracker backed by a SQL repository.
    pub fn from_repo(repo: storage::UsageRepo) -> Self {
        Self {
            data_dir: PathBuf::new(),
            sql_repo: Some(repo),
        }
    }

    fn usage_path(&self) -> PathBuf {
        self.data_dir.join("usage.jsonl")
    }

    /// Record a usage entry, appending to usage.jsonl or SQL.
    pub async fn record(
        &self,
        usage: &Usage,
        model: &str,
        provider: &str,
        strategy: &str,
        channel: &str,
    ) -> Result<()> {
        let cost = estimate_cost(usage, model);
        let request_id = uuid::Uuid::new_v4().to_string();

        // SQL path
        if let Some(repo) = &self.sql_repo {
            let row = storage::UsageRecordRow {
                id: uuid::Uuid::new_v4(),
                timestamp: Utc::now(),
                request_id,
                model: model.to_string(),
                provider: provider.to_string(),
                prompt_tokens: usage.prompt_tokens as i32,
                completion_tokens: usage.completion_tokens as i32,
                cache_read_tokens: usage.cache_read_tokens as i32,
                cache_write_tokens: usage.cache_write_tokens as i32,
                estimated_cost_usd: cost,
                channel: channel.to_string(),
                strategy: strategy.to_string(),
            };
            repo.create(&row)
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
            return Ok(());
        }

        // JSONL path (original)
        let record = UsageRecord {
            timestamp: Utc::now(),
            request_id,
            model: model.to_string(),
            provider: provider.to_string(),
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            estimated_cost_usd: cost,
            channel: channel.to_string(),
            strategy: strategy.to_string(),
        };

        let line = match serde_json::to_string(&record) {
            Ok(l) => l,
            Err(e) => {
                warn!("Failed to serialize usage record: {}", e);
                return Ok(());
            }
        };

        // Ensure data dir exists
        if let Err(e) = tokio::fs::create_dir_all(&self.data_dir).await {
            warn!("Failed to create data dir: {}", e);
            return Ok(());
        }

        // Atomic append: write to temp file, then append
        let path = self.usage_path();
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(format!("{}\n", line).as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// Generate a usage report for the last N days.
    pub async fn report(&self, days: u32) -> Result<UsageReport> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);

        // SQL path
        if let Some(repo) = &self.sql_repo {
            let (total_requests_i64, total_cost) = repo
                .totals_since(cutoff)
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

            let model_aggs = repo
                .aggregate_by_model(cutoff)
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

            let by_day = repo
                .aggregate_by_day(cutoff)
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

            let mut by_model: HashMap<String, (u64, f64)> = HashMap::new();
            let mut total_tokens = 0u64;
            for (model, tokens, cost) in model_aggs {
                total_tokens += tokens as u64;
                by_model.insert(model, (tokens as u64, cost));
            }

            return Ok(UsageReport {
                total_requests: total_requests_i64 as usize,
                total_tokens,
                total_cost_usd: total_cost,
                by_model,
                by_day,
            });
        }

        // JSONL path (original)
        let path = self.usage_path();

        let mut total_requests = 0usize;
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0f64;
        let mut by_model: HashMap<String, (u64, f64)> = HashMap::new();
        let mut by_day_map: HashMap<String, f64> = HashMap::new();

        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let record: UsageRecord = match serde_json::from_str(line) {
                    Ok(r) => r,
                    Err(_) => continue, // skip corrupt lines
                };

                if record.timestamp < cutoff {
                    continue;
                }

                total_requests += 1;
                let tokens = (record.prompt_tokens + record.completion_tokens) as u64;
                total_tokens += tokens;
                total_cost += record.estimated_cost_usd;

                let entry = by_model.entry(record.model.clone()).or_insert((0, 0.0));
                entry.0 += tokens;
                entry.1 += record.estimated_cost_usd;

                let day = record.timestamp.format("%Y-%m-%d").to_string();
                *by_day_map.entry(day).or_insert(0.0) += record.estimated_cost_usd;
            }
        }

        let mut by_day: Vec<(String, f64)> = by_day_map.into_iter().collect();
        by_day.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(UsageReport {
            total_requests,
            total_tokens,
            total_cost_usd: total_cost,
            by_model,
            by_day,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculation_claude_sonnet() {
        let usage = Usage {
            prompt_tokens: 1_000_000,   // 1M input tokens
            completion_tokens: 100_000, // 100K output tokens
            total_tokens: 1_100_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        let cost = estimate_cost(&usage, "claude-sonnet-4");
        // Input: 1M * $3/MTok = $3.00
        // Output: 0.1M * $15/MTok = $1.50
        assert!(
            (cost - 4.5).abs() < 0.001,
            "Expected $4.50, got ${:.4}",
            cost
        );
    }

    #[test]
    fn test_cost_calculation_claude_opus() {
        let usage = Usage {
            prompt_tokens: 500_000,
            completion_tokens: 50_000,
            total_tokens: 550_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        let cost = estimate_cost(&usage, "claude-opus-4");
        // Input: 0.5M * $15/MTok = $7.50
        // Output: 0.05M * $75/MTok = $3.75
        assert!(
            (cost - 11.25).abs() < 0.001,
            "Expected $11.25, got ${:.4}",
            cost
        );
    }

    #[test]
    fn test_unknown_model_zero_cost() {
        let usage = Usage {
            prompt_tokens: 1_000_000,
            completion_tokens: 500_000,
            total_tokens: 1_500_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        let cost = estimate_cost(&usage, "totally-unknown-model");
        assert_eq!(cost, 0.0, "Unknown model should have $0 cost");
    }

    #[test]
    fn test_model_pricing_haiku() {
        let (input, output) = model_pricing("claude-haiku-3");
        assert!((input - 0.25).abs() < 0.001);
        assert!((output - 1.25).abs() < 0.001);
    }

    #[test]
    fn test_model_pricing_gpt4o() {
        let (input, output) = model_pricing("gpt-4o-mini");
        assert!((input - 2.50).abs() < 0.001);
        assert!((output - 10.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_record_creates_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let tracker = CostTracker::new(dir.path().to_path_buf());

        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };

        tracker
            .record(
                &usage,
                "claude-sonnet-4",
                "anthropic",
                "tool_assisted",
                "telegram",
            )
            .await
            .unwrap();

        let path = dir.path().join("usage.jsonl");
        assert!(path.exists(), "usage.jsonl should be created");

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let record: UsageRecord = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(record.model, "claude-sonnet-4");
        assert_eq!(record.prompt_tokens, 100);
        assert_eq!(record.completion_tokens, 50);
        assert_eq!(record.channel, "telegram");
        assert!(record.estimated_cost_usd > 0.0);
    }

    #[tokio::test]
    async fn test_report_aggregates_by_model() {
        let dir = tempfile::tempdir().unwrap();
        let tracker = CostTracker::new(dir.path().to_path_buf());

        let usage1 = Usage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        let usage2 = Usage {
            prompt_tokens: 2000,
            completion_tokens: 1000,
            total_tokens: 3000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };

        tracker
            .record(&usage1, "claude-sonnet-4", "anthropic", "direct", "slack")
            .await
            .unwrap();
        tracker
            .record(
                &usage2,
                "claude-haiku-3",
                "anthropic",
                "tool_assisted",
                "telegram",
            )
            .await
            .unwrap();
        tracker
            .record(&usage1, "claude-sonnet-4", "anthropic", "direct", "discord")
            .await
            .unwrap();

        let report = tracker.report(30).await.unwrap();
        assert_eq!(report.total_requests, 3);
        assert_eq!(report.by_model.len(), 2, "Should have 2 distinct models");
        assert!(report.by_model.contains_key("claude-sonnet-4"));
        assert!(report.by_model.contains_key("claude-haiku-3"));

        // Sonnet had 2 records
        let (sonnet_tokens, _) = report.by_model["claude-sonnet-4"];
        assert_eq!(sonnet_tokens, 3000); // 1500 * 2
    }

    #[tokio::test]
    async fn test_report_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let tracker = CostTracker::new(dir.path().to_path_buf());
        let report = tracker.report(30).await.unwrap();
        assert_eq!(report.total_requests, 0);
        assert_eq!(report.total_cost_usd, 0.0);
    }
}
