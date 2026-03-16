//! Investment summary handler for `FinanceTool`.

use serde_json::json;

use common::{Result, ToolError};
use storage::rows::finance::FinanceInvestmentFilter;
use tools_core::ParamExtractor;

use super::super::FinanceTool;

impl FinanceTool {
    pub(super) async fn investment_summary(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.optional_str("portfolio_id")?;

        // Verify portfolio exists if specified
        if let Some(pid) = portfolio_id {
            let portfolio_exists = self.storage.investments.get_portfolio(pid).await?;
            if portfolio_exists.is_none() {
                return Err(
                    ToolError::ExecutionFailed(format!("Portfolio not found: {pid}")).into(),
                );
            }
        }

        let filter = FinanceInvestmentFilter {
            portfolio_id: portfolio_id.map(|s| s.to_string()),
            ..Default::default()
        };
        let investments = self.storage.investments.list_investments(&filter).await?;

        let total_cost_basis: i64 = investments.iter().map(|i| i.cost_basis).sum();
        // Use current_value if set; fall back to cost_basis so the summary is always populated
        let total_value: i64 = investments
            .iter()
            .map(|i| i.current_value.unwrap_or(i.cost_basis))
            .sum();
        let total_return = total_value - total_cost_basis;
        let return_pct = if total_cost_basis > 0 {
            (total_return * 100) / total_cost_basis
        } else {
            0
        };

        // Asset allocation grouped by asset_type
        let mut alloc_map: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        for inv in &investments {
            let value = inv.current_value.unwrap_or(inv.cost_basis);
            *alloc_map.entry(inv.asset_type.clone()).or_insert(0) += value;
        }
        let mut allocation: Vec<_> = alloc_map
            .iter()
            .map(|(asset_type, value)| {
                let pct = if total_value > 0 {
                    (value * 100) / total_value
                } else {
                    0
                };
                json!({"asset_type": asset_type, "value": value, "pct": pct})
            })
            .collect();
        allocation.sort_by(|a, b| {
            b["value"]
                .as_i64()
                .unwrap_or(0)
                .cmp(&a["value"].as_i64().unwrap_or(0))
        });

        let holdings: Vec<_> = investments
            .iter()
            .map(|inv| {
                let value = inv.current_value.unwrap_or(inv.cost_basis);
                let ret = value - inv.cost_basis;
                let ret_pct = if inv.cost_basis > 0 {
                    (ret * 100) / inv.cost_basis
                } else {
                    0
                };
                let inv_qty: f64 = inv.quantity_f64();
                json!({
                    "id": inv.id,
                    "name": inv.name,
                    "symbol": inv.symbol,
                    "asset_type": inv.asset_type,
                    "quantity": inv_qty,
                    "cost_basis": inv.cost_basis,
                    "current_value": value,
                    "return": ret,
                    "return_pct": ret_pct,
                    "currency": inv.currency,
                })
            })
            .collect();

        let resp = json!({
            "total_value": total_value,
            "total_cost_basis": total_cost_basis,
            "total_return": total_return,
            "return_pct": return_pct,
            "allocation": allocation,
            "holdings": holdings,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }
}
