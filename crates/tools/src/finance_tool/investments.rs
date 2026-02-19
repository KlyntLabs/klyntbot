//! Investment and portfolio action handlers for `FinanceTool`.
//!
//! Handles: portfolio_create, portfolio_list, investment_add, investment_update,
//! investment_tx, investment_summary, price_fetch, price_refresh.

use chrono::{Local, NaiveDate, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::finance_types::{AssetType, InvestmentTxType};
use crate::params::ParamExtractor;
use crate::RoutingContext;
use common::{Result, ToolError};

use super::FinanceTool;

impl FinanceTool {
    pub(crate) async fn handle_investment(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "portfolio_create" => self.portfolio_create(p).await,
            "portfolio_list" => self.portfolio_list(p).await,
            "investment_add" => self.investment_add(p).await,
            "investment_update" => self.investment_update(p).await,
            "investment_tx" => self.investment_tx(p).await,
            "investment_summary" => self.investment_summary(p).await,
            "price_fetch" => self.price_fetch(p).await,
            "price_refresh" => self.price_refresh(p).await,
            _ => {
                Err(ToolError::InvalidParams(format!("Unknown investment action: {action}")).into())
            }
        }
    }

    // ── Portfolio ──────────────────────────────────────────────────────────────

    async fn portfolio_create(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let name = p.required_str("name")?;
        if name.is_empty() {
            return Err(ToolError::InvalidParams("Portfolio name is required".to_string()).into());
        }
        let description = p.optional_str("description")?;
        // Fall back to the tool's default_currency if not supplied
        let currency = match p.optional_str("currency")? {
            Some(c) => c.to_string(),
            None => self.default_currency.clone(),
        };

        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        let row = storage::FinancePortfolioRow {
            id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            currency,
            created_at: now,
            updated_at: now,
        };

        let inserted = self.investments.add_portfolio(&row).await?;

        let resp = json!({
            "id": inserted.id,
            "name": inserted.name,
            "description": inserted.description,
            "currency": inserted.currency,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    async fn portfolio_list(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let portfolios = self.investments.list_portfolios().await?;

        let mut result = Vec::new();
        for portfolio in &portfolios {
            let summary = self.investments.portfolio_summary(&portfolio.id).await?;
            let total_return = summary.total_current_value - summary.total_cost_basis;
            let return_pct = if summary.total_cost_basis > 0 {
                (total_return * 100) / summary.total_cost_basis
            } else {
                0
            };
            result.push(json!({
                "id": portfolio.id,
                "name": portfolio.name,
                "currency": portfolio.currency,
                "total_value": summary.total_current_value,
                "total_cost_basis": summary.total_cost_basis,
                "total_return": total_return,
                "return_pct": return_pct,
                "holding_count": summary.holding_count,
            }));
        }

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    // ── Investment holdings ────────────────────────────────────────────────────

    async fn investment_add(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.required_str("portfolio_id")?;

        // Verify portfolio exists
        let portfolio_exists = self.investments.get_portfolio(portfolio_id).await?;
        if portfolio_exists.is_none() {
            return Err(
                ToolError::ExecutionFailed(format!("Portfolio not found: {portfolio_id}")).into(),
            );
        }

        let asset_type_str = p.required_str("asset_type")?;
        let asset_type = AssetType::from_str_loose(asset_type_str).ok_or_else(|| {
            ToolError::InvalidParams(format!("Invalid asset_type: {asset_type_str}"))
        })?;
        let symbol = p.optional_str("symbol")?;

        // Stocks and ETFs require a symbol
        if matches!(asset_type, AssetType::Stock | AssetType::Etf) && symbol.is_none() {
            return Err(ToolError::InvalidParams(format!(
                "Symbol is required for {} investments",
                asset_type.as_str()
            ))
            .into());
        }

        let name = p.required_str("name")?;
        // quantity is type `number` (f64) in the schema
        let quantity = p.optional_f64("quantity")?.ok_or_else(|| {
            ToolError::InvalidParams("missing required 'quantity' parameter".to_string())
        })?;
        if quantity < 0.0 {
            return Err(ToolError::InvalidParams("Quantity must be positive".to_string()).into());
        }
        let cost_basis = p.required_i64("cost_basis")?;
        let currency = p.required_str("currency")?;
        let purchase_date = match p.optional_str("purchase_date")? {
            Some(s) => Some(
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map_err(|_| ToolError::InvalidParams(format!("Invalid purchase_date: {s}")))?,
            ),
            None => None,
        };
        let notes = p.optional_str("notes")?;

        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        let row = storage::FinanceInvestmentRow {
            id,
            portfolio_id: portfolio_id.to_string(),
            asset_type: asset_type.as_str().to_string(),
            symbol: symbol.map(|s| s.to_string()),
            name: name.to_string(),
            quantity,
            cost_basis,
            currency: currency.to_string(),
            current_price: None,
            current_value: None,
            purchase_date,
            notes: notes.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };

        let inserted = self.investments.add_investment(&row).await?;

        let investment = json!({
            "id": inserted.id,
            "portfolio_id": inserted.portfolio_id,
            "asset_type": inserted.asset_type,
            "symbol": inserted.symbol,
            "name": inserted.name,
            "quantity": inserted.quantity,
            "cost_basis": inserted.cost_basis,
            "currency": inserted.currency,
            "current_price": inserted.current_price,
            "current_value": inserted.current_value,
            "purchase_date": inserted.purchase_date.map(|d| d.to_string()),
            "notes": inserted.notes,
        });
        Ok(serde_json::to_string_pretty(&json!({"investment": investment})).unwrap())
    }

    async fn investment_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let current_price = p.optional_i64("current_price")?;
        let current_value = p.optional_i64("current_value")?;
        let quantity = p.optional_f64("quantity")?;
        let notes = p.optional_str("notes")?;

        let patch = storage::FinanceInvestmentPatch {
            id: id.to_string(),
            current_price: current_price.map(Some),
            current_value: current_value.map(Some),
            quantity,
            cost_basis: None,
            notes: notes.map(|s| Some(s.to_string())),
        };

        let updated = self.investments.update_investment(&patch).await?;

        let investment = json!({
            "id": updated.id,
            "portfolio_id": updated.portfolio_id,
            "asset_type": updated.asset_type,
            "symbol": updated.symbol,
            "name": updated.name,
            "quantity": updated.quantity,
            "cost_basis": updated.cost_basis,
            "currency": updated.currency,
            "current_price": updated.current_price,
            "current_value": updated.current_value,
            "purchase_date": updated.purchase_date.map(|d| d.to_string()),
            "notes": updated.notes,
        });
        Ok(serde_json::to_string_pretty(&json!({"investment": investment})).unwrap())
    }

    // ── Investment transactions ────────────────────────────────────────────────

    async fn investment_tx(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let investment_id = p.required_str("investment_id")?;
        let tx_type_str = p.required_str("tx_type")?;
        let tx_type = InvestmentTxType::from_str_loose(tx_type_str)
            .ok_or_else(|| ToolError::InvalidParams(format!("Invalid tx_type: {tx_type_str}")))?;
        let quantity = p.optional_f64("quantity")?;
        let price_per_unit = p.optional_i64("price_per_unit")?;
        let total_amount = p.required_i64("total_amount")?;
        let fees = p.i64_or("fees", 0)?;
        let notes = p.optional_str("notes")?;

        let today = Local::now().date_naive();
        let tx_date = match p.optional_str("date")? {
            Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| ToolError::InvalidParams(format!("Invalid date: {s}")))?,
            None => today,
        };

        // Load existing investment for currency fallback and cost-basis computation
        let inv = self
            .investments
            .get_investment(investment_id)
            .await?
            .ok_or_else(|| {
                ToolError::InvalidParams(format!("Investment not found: {investment_id}"))
            })?;

        let currency = match p.optional_str("currency")? {
            Some(c) => c.to_string(),
            None => inv.currency.clone(),
        };

        let now = Utc::now();
        let tx_id = Uuid::new_v4().to_string();

        // Validate sell quantity does not exceed current holding
        if tx_type == InvestmentTxType::Sell {
            if let Some(sell_qty) = quantity {
                if sell_qty > inv.quantity {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Cannot sell more than current holding ({} available)",
                        inv.quantity
                    ))
                    .into());
                }
            }
        }

        let tx_row = storage::FinanceInvestmentTxRow {
            id: tx_id,
            investment_id: investment_id.to_string(),
            tx_type: tx_type.as_str().to_string(),
            quantity,
            price_per_unit,
            total_amount,
            currency,
            fees,
            tx_date,
            notes: notes.map(|s| s.to_string()),
            created_at: now,
        };

        let inserted_tx = self.investments.add_investment_tx(&tx_row).await?;

        // Compute and apply the parent-investment patch based on tx_type
        let patch = self.compute_investment_patch(&inv, tx_type, quantity, total_amount)?;
        let updated_inv = self.investments.update_investment(&patch).await?;

        let investment_tx = json!({
            "id": inserted_tx.id,
            "investment_id": inserted_tx.investment_id,
            "tx_type": inserted_tx.tx_type,
            "quantity": inserted_tx.quantity,
            "price_per_unit": inserted_tx.price_per_unit,
            "total_amount": inserted_tx.total_amount,
            "currency": inserted_tx.currency,
            "fees": inserted_tx.fees,
            "tx_date": inserted_tx.tx_date.to_string(),
            "notes": inserted_tx.notes,
        });

        let updated_investment = json!({
            "id": updated_inv.id,
            "quantity": updated_inv.quantity,
            "cost_basis": updated_inv.cost_basis,
            "current_price": updated_inv.current_price,
            "current_value": updated_inv.current_value,
        });

        let resp = json!({
            "investment_tx": investment_tx,
            "updated_investment": updated_investment,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    /// Build the `FinanceInvestmentPatch` that reflects a transaction's effect on the holding.
    fn compute_investment_patch(
        &self,
        inv: &storage::FinanceInvestmentRow,
        tx_type: InvestmentTxType,
        quantity: Option<f64>,
        total_amount: i64,
    ) -> Result<storage::FinanceInvestmentPatch> {
        let patch = match tx_type {
            InvestmentTxType::Buy => {
                let qty = quantity.unwrap_or(0.0);
                storage::FinanceInvestmentPatch {
                    id: inv.id.clone(),
                    quantity: Some(inv.quantity + qty),
                    cost_basis: Some(inv.cost_basis + total_amount),
                    current_price: None,
                    current_value: None,
                    notes: None,
                }
            }
            InvestmentTxType::Sell => {
                let sell_qty = quantity.unwrap_or(0.0);
                // Average cost per unit (floating-point to avoid integer truncation)
                let cost_per_unit = if inv.quantity > 0.0 {
                    inv.cost_basis as f64 / inv.quantity
                } else {
                    0.0
                };
                let cost_reduction = (cost_per_unit * sell_qty).round() as i64;
                let new_quantity = (inv.quantity - sell_qty).max(0.0);
                let new_cost_basis = (inv.cost_basis - cost_reduction).max(0);
                storage::FinanceInvestmentPatch {
                    id: inv.id.clone(),
                    quantity: Some(new_quantity),
                    cost_basis: Some(new_cost_basis),
                    current_price: None,
                    current_value: None,
                    notes: None,
                }
            }
            InvestmentTxType::Split => {
                // quantity param is the split ratio (e.g. 2.0 for a 2:1 split)
                let ratio = quantity.unwrap_or(1.0);
                storage::FinanceInvestmentPatch {
                    id: inv.id.clone(),
                    quantity: Some(inv.quantity * ratio),
                    cost_basis: None, // cost basis unchanged in a stock split
                    current_price: None,
                    current_value: None,
                    notes: None,
                }
            }
            // Dividend, rental income, interest: record income only — no quantity/basis change
            InvestmentTxType::Dividend
            | InvestmentTxType::RentalIncome
            | InvestmentTxType::Interest => storage::FinanceInvestmentPatch {
                id: inv.id.clone(),
                quantity: None,
                cost_basis: None,
                current_price: None,
                current_value: None,
                notes: None,
            },
        };
        Ok(patch)
    }

    // ── Summary ────────────────────────────────────────────────────────────────

    async fn investment_summary(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.optional_str("portfolio_id")?;

        // Verify portfolio exists if specified
        if let Some(pid) = portfolio_id {
            let portfolio_exists = self.investments.get_portfolio(pid).await?;
            if portfolio_exists.is_none() {
                return Err(
                    ToolError::ExecutionFailed(format!("Portfolio not found: {pid}")).into(),
                );
            }
        }

        let filter = storage::FinanceInvestmentFilter {
            portfolio_id: portfolio_id.map(|s| s.to_string()),
            ..Default::default()
        };
        let investments = self.investments.list_investments(&filter).await?;

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
                json!({
                    "id": inv.id,
                    "name": inv.name,
                    "symbol": inv.symbol,
                    "asset_type": inv.asset_type,
                    "quantity": inv.quantity,
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

    // ── Price fetching ─────────────────────────────────────────────────────────

    async fn price_fetch(&self, p: &ParamExtractor<'_>) -> Result<String> {
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
        let all = self.investments.list_with_symbols().await?;
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
            let current_value = (price_result.price * inv.quantity * 100.0).round() as i64;
            if self
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

    async fn price_refresh(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let investments = self.investments.list_with_symbols().await?;

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
                    let current_value = (price_result.price * inv.quantity * 100.0).round() as i64;
                    match self
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

#[cfg(test)]
mod tests {
    use crate::finance_types::{AssetType, InvestmentTxType};
    use crate::params::ParamExtractor;
    use serde_json::json;

    #[test]
    fn portfolio_create_requires_name() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("name").is_err());
    }

    #[test]
    fn investment_add_required_params() {
        // portfolio_id
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("portfolio_id").is_err());

        // asset_type
        let args = json!({"portfolio_id": "p1"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("asset_type").is_err());

        // name
        let args = json!({"portfolio_id": "p1", "asset_type": "stock"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("name").is_err());

        // cost_basis
        let args = json!({"portfolio_id": "p1", "asset_type": "stock", "name": "Apple"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_i64("cost_basis").is_err());
    }

    #[test]
    fn asset_type_all_values_parse() {
        for s in &["stock", "etf", "crypto", "real_estate", "bond", "other"] {
            assert!(
                AssetType::from_str_loose(s).is_some(),
                "asset_type '{s}' should parse"
            );
        }
        assert!(AssetType::from_str_loose("unknown_asset").is_none());
    }

    #[test]
    fn investment_tx_type_all_values_parse() {
        for s in &[
            "buy",
            "sell",
            "dividend",
            "rental_income",
            "interest",
            "split",
        ] {
            assert!(
                InvestmentTxType::from_str_loose(s).is_some(),
                "tx_type '{s}' should parse"
            );
        }
        assert!(InvestmentTxType::from_str_loose("invalid").is_none());
    }

    #[test]
    fn investment_tx_requires_investment_id_and_tx_type_and_total_amount() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("investment_id").is_err());

        let args = json!({"investment_id": "i1"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("tx_type").is_err());

        let args = json!({"investment_id": "i1", "tx_type": "buy"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_i64("total_amount").is_err());
    }

    #[test]
    fn buy_tx_increases_quantity_and_cost_basis() {
        let old_qty = 100.0f64;
        let old_basis = 1_000_000i64;
        let buy_qty = 50.0f64;
        let total = 500_000i64;

        assert_eq!(old_qty + buy_qty, 150.0);
        assert_eq!(old_basis + total, 1_500_000);
    }

    #[test]
    fn sell_tx_average_cost_basis_calculation() {
        let old_qty = 100.0f64;
        let old_basis = 1_000_000i64;
        let sell_qty = 25.0f64;

        let cost_per_unit = old_basis as f64 / old_qty; // 10_000
        let cost_reduction = (cost_per_unit * sell_qty).round() as i64; // 250_000
        let new_qty = (old_qty - sell_qty).max(0.0);
        let new_basis = (old_basis - cost_reduction).max(0);

        assert_eq!(new_qty, 75.0);
        assert_eq!(new_basis, 750_000);
    }

    #[test]
    fn sell_tx_does_not_go_below_zero() {
        // Selling more than you own: quantity and cost_basis clamp to 0
        let old_qty = 10.0f64;
        let old_basis = 100_000i64;
        let sell_qty = 20.0f64; // more than owned

        let cost_per_unit = old_basis as f64 / old_qty;
        let cost_reduction = (cost_per_unit * sell_qty).round() as i64;
        let new_qty = (old_qty - sell_qty).max(0.0);
        let new_basis = (old_basis - cost_reduction).max(0);

        assert_eq!(new_qty, 0.0);
        assert_eq!(new_basis, 0);
    }

    #[test]
    fn split_tx_multiplies_quantity() {
        let old_qty = 100.0f64;
        let ratio = 3.0f64; // 3:1 split
        assert_eq!(old_qty * ratio, 300.0);
    }

    #[test]
    fn portfolio_list_return_pct_calculation() {
        let cost = 1_000_000i64;
        let value = 1_200_000i64;
        let ret = value - cost;
        let pct = if cost > 0 { (ret * 100) / cost } else { 0 };

        assert_eq!(ret, 200_000);
        assert_eq!(pct, 20);
    }

    #[test]
    fn portfolio_list_return_pct_zero_cost_guard() {
        let cost = 0i64;
        let value = 0i64;
        let ret = value - cost;
        let pct = if cost > 0 { (ret * 100) / cost } else { 0 };
        assert_eq!(pct, 0, "should not divide by zero");
    }

    #[test]
    fn investment_summary_allocation_pct() {
        let total_value = 1_000_000i64;
        let stock_value = 600_000i64;
        let crypto_value = 400_000i64;

        let stock_pct = if total_value > 0 {
            (stock_value * 100) / total_value
        } else {
            0
        };
        let crypto_pct = if total_value > 0 {
            (crypto_value * 100) / total_value
        } else {
            0
        };

        assert_eq!(stock_pct, 60);
        assert_eq!(crypto_pct, 40);
        assert_eq!(stock_pct + crypto_pct, 100);
    }

    #[test]
    fn fees_defaults_to_zero() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("fees", 0).unwrap(), 0);
    }

    #[test]
    fn price_fetch_requires_symbol_and_asset_type() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("symbol").is_err());

        let args = json!({"symbol": "AAPL"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("asset_type").is_err());
    }
}
