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

/// Per-thread cost alert.
#[derive(Debug, Clone)]
pub struct CostAlert {
    pub session_key: String,
    pub spend_usd: f64,
    pub ceiling_usd: f64,
    pub percent: f64,
}

/// Tracks LLM usage and costs, persisted to SQL.
pub struct CostTracker {
    sql_repo: storage::UsageRepo,
    monthly_budget_usd: Option<f64>,
    session_ledger: dashmap::DashMap<String, f64>,
    last_reported_percent: dashmap::DashMap<String, u32>,
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
            session_ledger: dashmap::DashMap::new(),
            last_reported_percent: dashmap::DashMap::new(),
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

    /// Record usage for a specific session.
    pub fn record_for_session(&self, session_key: &str, usage: &Usage, model: &str) {
        let cost = estimate_cost(usage, model);
        *self
            .session_ledger
            .entry(session_key.to_string())
            .or_insert(0.0) += cost;
    }

    /// Total spend for a session.
    pub fn total_for_session(&self, session_key: &str) -> f64 {
        self.session_ledger
            .get(session_key)
            .map(|v| *v)
            .unwrap_or(0.0)
    }

    /// Check if a session has crossed its cost ceiling alert threshold.
    /// Returns `Some(CostAlert)` once per crossing.
    pub fn check_session_ceiling(
        &self,
        session_key: &str,
        ceiling_usd: f64,
        alert_at_percent: u32,
    ) -> Option<CostAlert> {
        if ceiling_usd <= 0.0 {
            return None;
        }
        let spend = self.total_for_session(session_key);
        let percent = (spend / ceiling_usd) * 100.0;
        let threshold = alert_at_percent as f64;
        if percent >= threshold {
            let last = self
                .last_reported_percent
                .get(session_key)
                .map(|v| *v)
                .unwrap_or(0);
            // 10-percent bands to avoid spam (80, 90, 100, …)
            let current_band = (percent as u32 / 10) * 10;
            if current_band > last {
                self.last_reported_percent
                    .insert(session_key.to_string(), current_band);
                Some(CostAlert {
                    session_key: session_key.to_string(),
                    spend_usd: spend,
                    ceiling_usd,
                    percent,
                })
            } else {
                None
            }
        } else {
            None
        }
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

    #[tokio::test]
    async fn test_session_ceiling_10pct_bands() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::UsageRepo::new(pool.inner().clone());
        let tracker = CostTracker::from_repo(repo);
        let sk = "session-1";
        let ceiling = 10.0;
        let alert_at = 50;

        // Spend $4.9 → 49% — below threshold
        tracker.session_ledger.insert(sk.to_string(), 4.9);
        assert!(tracker
            .check_session_ceiling(sk, ceiling, alert_at)
            .is_none());

        // Spend $5.1 → 51% — first alert at band 50
        tracker.session_ledger.insert(sk.to_string(), 5.1);
        let a1 = tracker
            .check_session_ceiling(sk, ceiling, alert_at)
            .unwrap();
        assert_eq!(a1.percent, 51.0);

        // Spend $5.9 → 59% — still in band 50, no duplicate alert
        tracker.session_ledger.insert(sk.to_string(), 5.9);
        assert!(tracker
            .check_session_ceiling(sk, ceiling, alert_at)
            .is_none());

        // Spend $6.1 → 61% — crosses into band 60
        tracker.session_ledger.insert(sk.to_string(), 6.1);
        let a2 = tracker
            .check_session_ceiling(sk, ceiling, alert_at)
            .unwrap();
        assert_eq!(a2.percent, 61.0);
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
