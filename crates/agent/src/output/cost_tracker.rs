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

/// Per-million-token pricing for a model.
struct ModelPricing {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

/// Get pricing for a model by exact ID match, then substring fallback.
fn model_pricing(model: &str) -> ModelPricing {
    let m = model.to_lowercase();

    // Exact match table (checked first)
    match m.as_str() {
        // Claude 4.x
        "claude-opus-4" | "claude-opus-4-20250514" => ModelPricing {
            input: 15.0,
            output: 75.0,
            cache_read: 1.50,
            cache_write: 18.75,
        },
        "claude-sonnet-4" | "claude-sonnet-4-20250514" => ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
            cache_write: 3.75,
        },
        // Claude 3.5
        "claude-3-5-sonnet-20241022" | "claude-3-5-sonnet-latest" => ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
            cache_write: 3.75,
        },
        "claude-3-5-haiku-20241022" | "claude-3-5-haiku-latest" => ModelPricing {
            input: 0.80,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        // Claude 3
        "claude-3-haiku-20240307" => ModelPricing {
            input: 0.25,
            output: 1.25,
            cache_read: 0.03,
            cache_write: 0.30,
        },
        // GPT-4o family
        "gpt-4o" | "gpt-4o-2024-11-20" | "gpt-4o-2024-08-06" => ModelPricing {
            input: 2.50,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" => ModelPricing {
            input: 0.15,
            output: 0.60,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        // Gemini
        "gemini-2.0-flash" | "gemini-2.0-flash-001" => ModelPricing {
            input: 0.10,
            output: 0.40,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        "gemini-1.5-pro" | "gemini-1.5-pro-002" => ModelPricing {
            input: 1.25,
            output: 5.0,
            cache_read: 0.315,
            cache_write: 0.0,
        },
        "gemini-1.5-flash" | "gemini-1.5-flash-002" => ModelPricing {
            input: 0.075,
            output: 0.30,
            cache_read: 0.01875,
            cache_write: 0.0,
        },
        // DeepSeek
        "deepseek-chat" | "deepseek-v3" => ModelPricing {
            input: 0.27,
            output: 1.10,
            cache_read: 0.07,
            cache_write: 0.0,
        },
        "deepseek-reasoner" | "deepseek-r1" => ModelPricing {
            input: 0.55,
            output: 2.19,
            cache_read: 0.14,
            cache_write: 0.0,
        },
        // Mistral
        "mistral-large-latest" => ModelPricing {
            input: 2.0,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        "mistral-small-latest" => ModelPricing {
            input: 0.10,
            output: 0.30,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        _ => substring_fallback(&m),
    }
}

/// Fallback pricing using substring matching for unknown exact model IDs.
fn substring_fallback(model: &str) -> ModelPricing {
    if model.contains("opus") {
        ModelPricing {
            input: 15.0,
            output: 75.0,
            cache_read: 1.50,
            cache_write: 18.75,
        }
    } else if model.contains("sonnet") {
        ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
            cache_write: 3.75,
        }
    } else if model.contains("haiku") {
        ModelPricing {
            input: 0.25,
            output: 1.25,
            cache_read: 0.03,
            cache_write: 0.30,
        }
    } else if model.contains("gpt-4o-mini") {
        ModelPricing {
            input: 0.15,
            output: 0.60,
            cache_read: 0.075,
            cache_write: 0.0,
        }
    } else if model.contains("gpt-4o") {
        ModelPricing {
            input: 2.50,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        }
    } else if model.contains("gemini") {
        ModelPricing {
            input: 0.10,
            output: 0.40,
            cache_read: 0.025,
            cache_write: 0.0,
        }
    } else if model.contains("deepseek") {
        ModelPricing {
            input: 0.27,
            output: 1.10,
            cache_read: 0.07,
            cache_write: 0.0,
        }
    } else {
        ModelPricing {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        }
    }
}

pub fn estimate_cost(usage: &Usage, model: &str) -> f64 {
    let pricing = model_pricing(model);
    let input_cost = (usage.prompt_tokens as f64 / 1_000_000.0) * pricing.input;
    let output_cost = (usage.completion_tokens as f64 / 1_000_000.0) * pricing.output;
    let cache_read_cost = (usage.cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_read;
    let cache_write_cost = (usage.cache_write_tokens as f64 / 1_000_000.0) * pricing.cache_write;
    input_cost + output_cost + cache_read_cost + cache_write_cost
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
        let pricing = model_pricing("claude-haiku-3");
        assert!((pricing.input - 0.25).abs() < 0.001);
        assert!((pricing.output - 1.25).abs() < 0.001);
    }

    #[test]
    fn test_model_pricing_gpt4o() {
        let pricing = model_pricing("gpt-4o");
        assert!((pricing.input - 2.50).abs() < 0.001);
        assert!((pricing.output - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_model_pricing_gpt4o_mini_separate() {
        let pricing = model_pricing("gpt-4o-mini");
        assert!((pricing.input - 0.15).abs() < 0.001);
        assert!((pricing.output - 0.60).abs() < 0.001);
    }

    #[test]
    fn test_gpt4o_mini_has_own_pricing() {
        let pricing = model_pricing("gpt-4o-mini");
        // gpt-4o-mini should NOT match gpt-4o pricing
        assert!(
            pricing.input < 1.0,
            "gpt-4o-mini input should be < $1/MTok, got {}",
            pricing.input
        );
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
            cost > 0.45,
            "Cost should include cache tokens, got {}",
            cost
        );
    }

    #[test]
    fn test_gemini_pricing_exists() {
        let pricing = model_pricing("gemini-2.0-flash");
        assert!(pricing.input > 0.0, "Gemini should have non-zero pricing");
    }

    #[test]
    fn test_deepseek_pricing_exists() {
        let pricing = model_pricing("deepseek-chat");
        assert!(pricing.input > 0.0, "DeepSeek should have non-zero pricing");
    }

    #[test]
    fn test_substring_fallback_still_works_for_unknown_claude_variants() {
        // An unknown Claude variant should fall through to substring matching
        let pricing = model_pricing("claude-future-opus-xyz");
        assert!(
            pricing.input > 0.0,
            "Unknown opus variant should get opus pricing via fallback"
        );
    }

    #[test]
    fn test_cache_read_rate_for_sonnet() {
        let pricing = model_pricing("claude-sonnet-4");
        assert!(
            (pricing.cache_read - 0.30).abs() < 0.001,
            "claude-sonnet-4 cache_read should be $0.30/MTok"
        );
        assert!(
            (pricing.cache_write - 3.75).abs() < 0.001,
            "claude-sonnet-4 cache_write should be $3.75/MTok"
        );
    }
}
