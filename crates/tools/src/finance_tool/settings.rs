//! Settings action handlers for `FinanceTool`.
//!
//! Handles: settings_get, settings_update.

use serde_json::json;

use crate::params::ParamExtractor;
use crate::RoutingContext;
use common::{Result, ToolError};

use super::FinanceTool;

impl FinanceTool {
    pub(crate) async fn handle_settings(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "settings_get" => self.settings_get().await,
            "settings_update" => self.settings_update(p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown settings action: {action}")).into()),
        }
    }

    // ── settings_get ─────────────────────────────────────────────────────────

    async fn settings_get(&self) -> Result<String> {
        let cfg = config::load()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let finance_json = serde_json::to_value(&cfg.finance)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(serde_json::to_string_pretty(&finance_json).unwrap())
    }

    // ── settings_update ──────────────────────────────────────────────────────

    async fn settings_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let default_currency = p.optional_str("default_currency")?;
        let proactivity_level = p.optional_str("proactivity_level")?;
        let inflation_rate = p.optional_f64("inflation_rate")?;
        let alert_threshold = p.optional_i64("alert_threshold")?;
        let auto_categorize = p.optional_bool("auto_categorize")?;
        let confidence_threshold = p.optional_f64("confidence_threshold")?;

        if default_currency.is_none()
            && proactivity_level.is_none()
            && inflation_rate.is_none()
            && alert_threshold.is_none()
            && auto_categorize.is_none()
            && confidence_threshold.is_none()
        {
            return Err(
                ToolError::InvalidParams("No settings provided to update".to_string()).into(),
            );
        }

        let mut cfg = config::load()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let mut updated = serde_json::Map::new();

        if let Some(currency) = default_currency {
            let upper = currency.to_uppercase();
            cfg.finance.default_currency = upper.clone();
            updated.insert("default_currency".to_string(), json!(upper));
        }

        if let Some(level) = proactivity_level {
            match level {
                "full" | "moderate" | "reactive" => {}
                _ => {
                    return Err(ToolError::InvalidParams(format!(
                        "Invalid proactivity level: {}. Use: full, moderate, reactive",
                        level
                    ))
                    .into());
                }
            }
            cfg.finance.proactivity_level = level.to_string();
            updated.insert("proactivity_level".to_string(), json!(level));
        }

        if let Some(rate) = inflation_rate {
            cfg.finance.inflation.rate = rate;
            updated.insert("inflation_rate".to_string(), json!(rate));
        }

        if let Some(threshold) = alert_threshold {
            let clamped = threshold.clamp(0, 100) as u8;
            cfg.finance.budgeting.alert_threshold = clamped;
            updated.insert("alert_threshold".to_string(), json!(clamped));
        }

        if let Some(auto_cat) = auto_categorize {
            cfg.finance.categories.auto_categorize = auto_cat;
            updated.insert("auto_categorize".to_string(), json!(auto_cat));
        }

        if let Some(conf) = confidence_threshold {
            if !(0.0..=1.0).contains(&conf) {
                return Err(ToolError::InvalidParams(
                    "Confidence threshold must be between 0.0 and 1.0".to_string(),
                )
                .into());
            }
            cfg.finance.categories.confidence_threshold = conf;
            updated.insert("confidence_threshold".to_string(), json!(conf));
        }

        config::save(&cfg)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let result = json!({
            "status": "saved",
            "message": "Settings saved. Changes take effect on next restart.",
            "updated": serde_json::Value::Object(updated),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }
}
