//! Cost and usage tracker — records per-request LLM usage to SQL.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use common::Result;
use providers::Usage;

/// A single usage record.
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

/// Tracks LLM usage and costs, persisted to SQL.
pub struct CostTracker {
    sql_repo: storage::UsageRepo,
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
    /// Create a CostTracker backed by a SQL repository.
    pub fn from_repo(repo: storage::UsageRepo) -> Self {
        Self { sql_repo: repo }
    }

    /// Record a usage entry to SQL.
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
        self.sql_repo
            .create(&row)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        Ok(())
    }

    /// Generate a usage report for the last N days.
    pub async fn report(&self, days: u32) -> Result<UsageReport> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);

        let (total_requests_i64, total_cost) = self
            .sql_repo
            .totals_since(cutoff)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let model_aggs = self
            .sql_repo
            .aggregate_by_model(cutoff)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let by_day = self
            .sql_repo
            .aggregate_by_day(cutoff)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let mut by_model: HashMap<String, (u64, f64)> = HashMap::new();
        let mut total_tokens = 0u64;
        for (model, tokens, cost) in model_aggs {
            total_tokens += tokens as u64;
            by_model.insert(model, (tokens as u64, cost));
        }

        Ok(UsageReport {
            total_requests: total_requests_i64 as usize,
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
}
