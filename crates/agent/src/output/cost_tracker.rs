//! Cost and usage tracker — records per-request LLM usage to SQL.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use common::Result;
use providers::Usage;

/// A single usage record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: Timestamp,
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

/// Budget check result when monthly spend exceeds a threshold.
#[derive(Debug, Clone)]
pub struct BudgetAlert {
    pub monthly_spend_usd: f64,
    pub monthly_budget_usd: f64,
    pub usage_percent: f64,
}

/// Tracks LLM usage and costs, persisted to SQL.
pub struct CostTracker {
    sql_repo: storage::UsageRepo,
    monthly_budget_usd: Option<f64>,
}

pub fn estimate_cost(usage: &Usage, model: &str) -> f64 {
    common::pricing::cost_with_cache_for(
        model,
        usage.prompt_tokens as u64,
        usage.completion_tokens as u64,
        usage.cache_read_tokens as u64,
        usage.cache_write_tokens as u64,
    )
    .unwrap_or(0.0)
}

impl CostTracker {
    /// Create a CostTracker backed by a SQL repository.
    pub fn from_repo(repo: storage::UsageRepo) -> Self {
        Self {
            sql_repo: repo,
            monthly_budget_usd: None,
        }
    }

    /// Set a monthly budget threshold. Warnings emitted at 80% and 100%.
    pub fn with_monthly_budget(mut self, budget: Option<f64>) -> Self {
        self.monthly_budget_usd = budget;
        self
    }

    /// Check if monthly spend has crossed a budget threshold (80% or 100%).
    /// Returns a `BudgetAlert` if the current spend exceeds a warning level.
    pub async fn check_budget(&self) -> Option<BudgetAlert> {
        let budget = self.monthly_budget_usd?;
        if budget <= 0.0 {
            return None;
        }
        let spend = self.sql_repo.total_cost_current_month().await.ok()?;
        let pct = (spend / budget) * 100.0;
        if pct >= 80.0 {
            Some(BudgetAlert {
                monthly_spend_usd: spend,
                monthly_budget_usd: budget,
                usage_percent: pct,
            })
        } else {
            None
        }
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
            timestamp: Timestamp::now().into(),
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
        let cutoff = Timestamp::now()
            .checked_sub(jiff::SignedDuration::from_secs(days as i64 * 86400))
            .unwrap();

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
    fn test_cache_tokens_included_in_cost() {
        let usage = Usage {
            prompt_tokens: 100_000,
            completion_tokens: 10_000,
            total_tokens: 110_000,
            cache_read_tokens: 50_000,
            cache_write_tokens: 20_000,
        };
        let cost = estimate_cost(&usage, "claude-sonnet-4");
        // Without cache: input 0.1M * $3 + output 0.01M * $15 = $0.30 + $0.15 = $0.45
        // Cache read: 0.05M * $0.30 = $0.015
        // Cache write: 0.02M * $3.75 = $0.075
        // Total: $0.54
        assert!(
            (cost - 0.54).abs() < 0.001,
            "Cost should include cache tokens, got {}",
            cost
        );
    }
}
