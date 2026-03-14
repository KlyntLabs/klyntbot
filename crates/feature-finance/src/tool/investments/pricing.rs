//! Price fetch and refresh handlers for `FinanceTool`.

use serde_json::json;

use crate::types::AssetType;
use common::{Result, ToolError};
use tools_core::ParamExtractor;

use super::super::FinanceTool;

impl FinanceTool {
    pub(super) async fn price_fetch(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let symbol = p.required_str("symbol")?;
        let asset_type_str = p.required_str("asset_type")?;
        let asset_type = AssetType::from_str_loose(asset_type_str).ok_or_else(|| {
            ToolError::InvalidParams(format!("Invalid asset_type: {asset_type_str}"))
        })?;

        if asset_type == AssetType::RealEstate {
            return Err(ToolError::ExecutionFailed(
                "Price fetch is not available for real estate".to_string(),
            )
            .into());
        }

        let price_result = self
            .price_service
            .fetch_price(symbol, asset_type)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Price fetch failed: {e}")))?;

        // Update all investments whose symbol matches (case-insensitive)
        let all = self.storage.investments.list_with_symbols().await?;
        let matching: Vec<_> = all
            .iter()
            .filter(|i| {
                i.symbol
                    .as_deref()
                    .map(|s| s.to_lowercase() == symbol.to_lowercase())
                    .unwrap_or(false)
            })
            .collect();

        // Prices stored in smallest currency unit (×100 for 2-decimal currencies)
        let price_int = (price_result.price * 100.0).round() as i64;
        let mut updated_count = 0usize;
        for inv in &matching {
            let inv_qty: f64 = inv.quantity_f64();
            let current_value = (price_result.price * inv_qty * 100.0).round() as i64;
            if self
                .storage
                .investments
                .update_price(&inv.id, price_int, current_value)
                .await
                .is_ok()
            {
                updated_count += 1;
            }
        }

        let resp = json!({
            "symbol": price_result.symbol,
            "price": price_result.price,
            "currency": price_result.currency,
            "source": price_result.source,
            "updated_investments_count": updated_count,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    pub(super) async fn price_refresh(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let investments = self.storage.investments.list_with_symbols().await?;

        let mut updated = 0usize;
        let mut failed = 0usize;
        let mut details = Vec::new();

        for inv in &investments {
            let symbol = match &inv.symbol {
                Some(s) => s.clone(),
                None => continue,
            };

            let asset_type = AssetType::from_str_loose(&inv.asset_type).unwrap_or_default();
            match self.price_service.fetch_price(&symbol, asset_type).await {
                Ok(price_result) => {
                    let price_int = (price_result.price * 100.0).round() as i64;
                    let inv_qty: f64 = inv.quantity_f64();
                    let current_value = (price_result.price * inv_qty * 100.0).round() as i64;
                    match self
                        .storage
                        .investments
                        .update_price(&inv.id, price_int, current_value)
                        .await
                    {
                        Ok(_) => {
                            updated += 1;
                            details.push(json!({
                                "id": inv.id,
                                "symbol": symbol,
                                "price": price_result.price,
                                "status": "updated",
                            }));
                        }
                        Err(e) => {
                            failed += 1;
                            details.push(json!({
                                "id": inv.id,
                                "symbol": symbol,
                                "status": "failed",
                                "error": e.to_string(),
                            }));
                        }
                    }
                }
                Err(e) => {
                    failed += 1;
                    details.push(json!({
                        "id": inv.id,
                        "symbol": symbol,
                        "status": "failed",
                        "error": e,
                    }));
                }
            }
        }

        let resp = json!({
            "updated": updated,
            "failed": failed,
            "details": details,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }
}
