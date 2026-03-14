//! Allocation target action handlers for `FinanceTool`.
//!
//! Handles: allocation_target_set, allocation_target_list.

use common::{Decimal, Result, ToolError};
use serde_json::json;
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::FinanceTool;

impl FinanceTool {
    pub(crate) async fn handle_allocation(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "allocation_target_set" => self.allocation_target_set(p).await,
            "allocation_target_list" => self.allocation_target_list(p).await,
            _ => {
                Err(ToolError::InvalidParams(format!("Unknown allocation action: {action}")).into())
            }
        }
    }

    async fn allocation_target_set(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.required_str("portfolio_id")?;
        let asset_class = p.required_str("asset_class")?;
        let target_weight = p.optional_f64("target_weight")?.ok_or_else(|| {
            ToolError::InvalidParams("missing required 'target_weight' parameter".to_string())
        })?;
        let tolerance_band = p.optional_f64("tolerance_band")?.unwrap_or(0.05);

        // Validate weight is between 0 and 1
        if !(0.0..=1.0).contains(&target_weight) {
            return Err(ToolError::InvalidParams(
                "target_weight must be between 0.0 and 1.0".into(),
            )
            .into());
        }
        if !(0.0..=1.0).contains(&tolerance_band) {
            return Err(ToolError::InvalidParams(
                "tolerance_band must be between 0.0 and 1.0".into(),
            )
            .into());
        }

        // Verify portfolio exists
        let portfolio = self.storage.investments.get_portfolio(portfolio_id).await?;
        if portfolio.is_none() {
            return Err(ToolError::ExecutionFailed(format!(
                "Portfolio '{portfolio_id}' not found"
            ))
            .into());
        }

        let weight_str = Decimal::from_f64_retain(target_weight)
            .unwrap_or(Decimal::ZERO)
            .to_string();
        let band_str = Decimal::from_f64_retain(tolerance_band)
            .unwrap_or(Decimal::ZERO)
            .to_string();

        let row = self
            .storage
            .allocations
            .add(portfolio_id, asset_class, &weight_str, &band_str)
            .await?;

        Ok(serde_json::to_string_pretty(&json!({
            "id": row.id,
            "portfolio_id": row.portfolio_id,
            "asset_class": row.asset_class,
            "target_weight": row.target_weight,
            "tolerance_band": row.tolerance_band,
        }))
        .unwrap())
    }

    async fn allocation_target_list(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.required_str("portfolio_id")?;

        let rows = self
            .storage
            .allocations
            .list_by_portfolio(portfolio_id)
            .await?;

        if rows.is_empty() {
            return Ok(format!(
                "No allocation targets set for portfolio '{portfolio_id}'."
            ));
        }

        let targets: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "id": row.id,
                    "portfolio_id": row.portfolio_id,
                    "asset_class": row.asset_class,
                    "target_weight": row.target_weight,
                    "tolerance_band": row.tolerance_band,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&json!({
            "portfolio_id": portfolio_id,
            "allocation_targets": targets,
        }))
        .unwrap())
    }
}
