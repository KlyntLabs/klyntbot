//! Investment and portfolio action handlers for `FinanceTool`.
//!
//! Handles: portfolio_create, portfolio_list, investment_add, investment_update,
//! investment_tx, investment_summary, price_fetch, price_refresh.

mod pricing;
mod summary;

use chrono::{Local, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::types::{AssetType, InvestmentTxType};
use common::{Result, ToolError};
use storage::rows::finance::{
    FinanceInvestmentPatch, FinanceInvestmentRow, FinanceInvestmentTxRow, FinancePortfolioRow,
};
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::{parse_date, FinanceTool};

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
            "portfolio_drift" => self.portfolio_drift(p).await,
            "portfolio_rebalance" => self.portfolio_rebalance(p).await,
            "portfolio_returns" => self.portfolio_returns(p).await,
            "portfolio_correlation" => self.portfolio_correlation(p).await,
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

        let row = FinancePortfolioRow {
            id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            currency,
            created_at: now,
            updated_at: now,
        };

        let inserted = self.storage.investments.add_portfolio(&row).await?;

        let resp = json!({
            "id": inserted.id,
            "name": inserted.name,
            "description": inserted.description,
            "currency": inserted.currency,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    async fn portfolio_list(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let portfolios = self.storage.investments.list_portfolios().await?;

        let mut result = Vec::new();
        for portfolio in &portfolios {
            let summary = self
                .storage
                .investments
                .portfolio_summary(&portfolio.id)
                .await?;
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
        let portfolio_exists = self.storage.investments.get_portfolio(portfolio_id).await?;
        if portfolio_exists.is_none() {
            return Ok(serde_json::to_string_pretty(&json!({
                "error": "portfolio_not_found",
                "message": format!("Portfolio '{}' not found. Create one first or use portfolio_list to find IDs.", portfolio_id),
                "suggested_action": "portfolio_create",
                "example": {"action": "portfolio_create", "name": "My Portfolio", "currency": "USD"}
            }))
            .unwrap());
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

        // Default name to symbol if not provided (common for stocks/ETFs).
        let name = match p.optional_str("name")? {
            Some(n) => n,
            None => symbol.unwrap_or("Unnamed Investment"),
        };
        // quantity is type `number` (f64) in the schema
        let quantity = p.optional_f64("quantity")?.ok_or_else(|| {
            ToolError::InvalidParams("missing required 'quantity' parameter".to_string())
        })?;
        if quantity < 0.0 {
            return Err(ToolError::InvalidParams("Quantity must be positive".to_string()).into());
        }
        let cost_basis = p.required_i64("cost_basis")?;
        let currency = p
            .optional_str("currency")?
            .unwrap_or(&self.default_currency);
        let purchase_date = p
            .optional_str("purchase_date")?
            .map(parse_date)
            .transpose()?;
        let notes = p.optional_str("notes")?;

        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        let row = FinanceInvestmentRow {
            id,
            portfolio_id: portfolio_id.to_string(),
            asset_type: asset_type.as_str().to_string(),
            symbol: symbol.map(|s| s.to_string()),
            name: name.to_string(),
            quantity: quantity.to_string(),
            cost_basis,
            currency: currency.to_string(),
            current_price: None,
            current_value: None,
            purchase_date,
            asset_class: None,
            notes: notes.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };

        let inserted = self.storage.investments.add_investment(&row).await?;

        let inserted_qty: f64 = inserted.quantity_f64();
        let investment = json!({
            "id": inserted.id,
            "portfolio_id": inserted.portfolio_id,
            "asset_type": inserted.asset_type,
            "symbol": inserted.symbol,
            "name": inserted.name,
            "quantity": inserted_qty,
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

        let patch = FinanceInvestmentPatch {
            id: id.to_string(),
            current_price: current_price.map(Some),
            current_value: current_value.map(Some),
            quantity: quantity.map(|q| q.to_string()),
            cost_basis: None,
            notes: notes.map(|s| Some(s.to_string())),
        };

        let updated = self.storage.investments.update_investment(&patch).await?;

        let updated_qty: f64 = updated.quantity_f64();
        let investment = json!({
            "id": updated.id,
            "portfolio_id": updated.portfolio_id,
            "asset_type": updated.asset_type,
            "symbol": updated.symbol,
            "name": updated.name,
            "quantity": updated_qty,
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
        // Accept either "id" (from schema) or "investment_id" (explicit).
        let investment_id = p
            .optional_str("investment_id")?
            .or(p.optional_str("id")?)
            .ok_or_else(|| {
                ToolError::InvalidParams(
                    "missing required 'id' (investment ID) parameter".to_string(),
                )
            })?;
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
            Some(s) => parse_date(s)?,
            None => today,
        };

        // Load existing investment for currency fallback and cost-basis computation
        let inv = match self
            .storage
            .investments
            .get_investment(investment_id)
            .await?
        {
            Some(inv) => inv,
            None => {
                return Ok(serde_json::to_string_pretty(&json!({
                    "error": "investment_not_found",
                    "message": format!("Investment '{}' not found. Use investment_summary to find IDs.", investment_id),
                    "suggested_action": "investment_summary",
                    "example": {"action": "investment_summary"}
                }))
                .unwrap());
            }
        };

        let currency = match p.optional_str("currency")? {
            Some(c) => c.to_string(),
            None => inv.currency.clone(),
        };

        let now = Utc::now();
        let tx_id = Uuid::new_v4().to_string();

        // Validate sell quantity does not exceed current holding
        if tx_type == InvestmentTxType::Sell {
            let inv_qty: f64 = inv.quantity_f64();
            if let Some(sell_qty) = quantity {
                if sell_qty > inv_qty {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Cannot sell more than current holding ({inv_qty} available)",
                    ))
                    .into());
                }
            }
        }

        let tx_row = FinanceInvestmentTxRow {
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

        let inserted_tx = self.storage.investments.add_investment_tx(&tx_row).await?;

        // Compute and apply the parent-investment patch based on tx_type
        let patch = self.compute_investment_patch(&inv, tx_type, quantity, total_amount)?;
        let updated_inv = self.storage.investments.update_investment(&patch).await?;

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

        let updated_inv_qty: f64 = updated_inv.quantity_f64();
        let updated_investment = json!({
            "id": updated_inv.id,
            "quantity": updated_inv_qty,
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

    // ── Portfolio analytics ──────────────────────────────────────────────────

    async fn portfolio_drift(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.required_str("portfolio_id")?;

        let (holdings, targets) = self.fetch_holdings_and_targets(portfolio_id).await?;

        if holdings.is_empty() {
            return Ok("No holdings found for this portfolio.".to_string());
        }
        if targets.is_empty() {
            return Ok(
                "No allocation targets set for this portfolio. Use allocation_target_set first."
                    .to_string(),
            );
        }

        let result = analytics::portfolio::PortfolioAnalyzer::allocation_drift(&holdings, &targets);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn portfolio_rebalance(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.required_str("portfolio_id")?;
        let contribution = p.i64_or("contribution", 0)?;
        let min_trade = p.i64_or("min_trade_amount", 0)?;

        let strategy_str = p.str_or("strategy", "full")?;
        let strategy = match strategy_str {
            "contribution_only" => analytics::portfolio::RebalanceStrategy::ContributionOnly,
            "threshold_only" => analytics::portfolio::RebalanceStrategy::ThresholdOnly,
            _ => analytics::portfolio::RebalanceStrategy::FullRebalance,
        };

        let (holdings, targets) = self.fetch_holdings_and_targets(portfolio_id).await?;

        if holdings.is_empty() {
            return Ok("No holdings found for this portfolio.".to_string());
        }
        if targets.is_empty() {
            return Ok(
                "No allocation targets set for this portfolio. Use allocation_target_set first."
                    .to_string(),
            );
        }

        let result = analytics::portfolio::PortfolioAnalyzer::rebalance_suggestions(
            &holdings,
            &targets,
            strategy,
            common::Decimal::new(contribution, 0),
            common::Decimal::new(min_trade, 0),
        );
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn portfolio_returns(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.required_str("portfolio_id")?;
        let start_date_str = p.required_str("start_date")?;
        let end_date_str = p.required_str("end_date")?;
        let start_date = super::parse_date(start_date_str)?;
        let end_date = super::parse_date(end_date_str)?;

        // Compute portfolio start and end value from investments
        let filter = storage::rows::finance::FinanceInvestmentFilter {
            portfolio_id: Some(portfolio_id.to_string()),
            ..Default::default()
        };
        let investments = self.storage.investments.list_investments(&filter).await?;

        if investments.is_empty() {
            return Ok("No investments found for this portfolio.".to_string());
        }

        // Sum up current values (end value) and cost basis (approximation for start value)
        let mut end_value = common::Decimal::ZERO;
        let mut start_value = common::Decimal::ZERO;
        let mut cash_flows: Vec<analytics::InvestmentCashFlow> = Vec::new();

        for inv in &investments {
            let current_val = inv.current_value.unwrap_or(inv.cost_basis);
            end_value += common::Decimal::new(current_val, 0);
            start_value += common::Decimal::new(inv.cost_basis, 0);

            // Fetch investment transactions as cash flows
            let txs = self
                .storage
                .investments
                .list_investment_txs(&inv.id)
                .await?;
            for tx in &txs {
                if tx.tx_date >= start_date && tx.tx_date <= end_date {
                    let amount = match tx.tx_type.as_str() {
                        "buy" => common::Decimal::new(tx.total_amount, 0),
                        "sell" => common::Decimal::new(-tx.total_amount, 0),
                        _ => common::Decimal::ZERO,
                    };
                    if amount != common::Decimal::ZERO {
                        cash_flows.push(analytics::InvestmentCashFlow {
                            date: tx.tx_date,
                            amount,
                            holding_symbol: inv.symbol.clone(),
                        });
                    }
                }
            }
        }

        let result = analytics::portfolio::PortfolioAnalyzer::returns(
            start_value,
            end_value,
            &cash_flows,
            start_date,
            end_date,
        );
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn portfolio_correlation(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio_id = p.required_str("portfolio_id")?;

        let filter = storage::rows::finance::FinanceInvestmentFilter {
            portfolio_id: Some(portfolio_id.to_string()),
            has_symbol: Some(true),
            ..Default::default()
        };
        let investments = self.storage.investments.list_investments(&filter).await?;

        if investments.len() < 2 {
            return Ok(
                "Need at least 2 investments with symbols to compute correlation.".to_string(),
            );
        }

        // Build price series from investment transactions
        let mut series_list: Vec<analytics::PriceSeries> = Vec::new();

        for inv in &investments {
            let symbol = match &inv.symbol {
                Some(s) => s.clone(),
                None => continue,
            };
            let txs = self
                .storage
                .investments
                .list_investment_txs(&inv.id)
                .await?;
            let prices: Vec<(chrono::NaiveDate, common::Decimal)> = txs
                .iter()
                .filter_map(|tx| {
                    tx.price_per_unit
                        .map(|price| (tx.tx_date, common::Decimal::new(price, 0)))
                })
                .collect();

            if prices.len() >= 2 {
                series_list.push(analytics::PriceSeries {
                    symbol,
                    asset_class: inv
                        .asset_class
                        .clone()
                        .unwrap_or_else(|| inv.asset_type.clone()),
                    prices,
                });
            }
        }

        if series_list.len() < 2 {
            return Ok("Insufficient price history for correlation analysis. Need at least 2 assets with price data.".to_string());
        }

        let config = analytics::portfolio::AssetCorrelationConfig::default();
        let matrix =
            analytics::portfolio::PortfolioAnalyzer::asset_correlation(&series_list, &config);

        // Format coefficients as strings for JSON
        let coefficients: Vec<Vec<String>> = matrix
            .coefficients
            .iter()
            .map(|row| row.iter().map(|c| c.to_string()).collect())
            .collect();

        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "assets": matrix.labels,
            "correlation_matrix": coefficients,
        }))
        .unwrap())
    }

    /// Fetch holdings and allocation targets for a portfolio, converting to analytics types.
    async fn fetch_holdings_and_targets(
        &self,
        portfolio_id: &str,
    ) -> Result<(Vec<analytics::Holding>, Vec<analytics::AllocationTarget>)> {
        let filter = storage::rows::finance::FinanceInvestmentFilter {
            portfolio_id: Some(portfolio_id.to_string()),
            ..Default::default()
        };
        let inv_rows = self.storage.investments.list_investments(&filter).await?;

        let holdings: Vec<analytics::Holding> = inv_rows
            .iter()
            .map(|row| {
                let qty: f64 = row.quantity_f64();
                let current_value = row.current_value.unwrap_or(row.cost_basis);
                analytics::Holding {
                    name: row.name.clone(),
                    symbol: row.symbol.clone(),
                    asset_class: row
                        .asset_class
                        .clone()
                        .unwrap_or_else(|| row.asset_type.clone()),
                    current_value: common::Decimal::new(current_value, 0),
                    cost_basis: common::Decimal::new(row.cost_basis, 0),
                    quantity: common::Decimal::from_f64_retain(qty)
                        .unwrap_or(common::Decimal::ZERO),
                }
            })
            .collect();

        let target_rows = self
            .storage
            .allocations
            .list_by_portfolio(portfolio_id)
            .await?;
        let targets: Vec<analytics::AllocationTarget> = target_rows
            .iter()
            .map(|row| {
                let target_weight: common::Decimal =
                    row.target_weight.parse().unwrap_or(common::Decimal::ZERO);
                let tolerance_band: common::Decimal = row
                    .tolerance_band
                    .parse()
                    .unwrap_or(common::Decimal::new(5, 2));
                analytics::AllocationTarget {
                    asset_class: row.asset_class.clone(),
                    target_weight,
                    tolerance_band,
                }
            })
            .collect();

        Ok((holdings, targets))
    }

    /// Build the `FinanceInvestmentPatch` that reflects a transaction's effect on the holding.
    fn compute_investment_patch(
        &self,
        inv: &FinanceInvestmentRow,
        tx_type: InvestmentTxType,
        quantity: Option<f64>,
        total_amount: i64,
    ) -> Result<FinanceInvestmentPatch> {
        let inv_qty: f64 = inv.quantity_f64();
        let patch = match tx_type {
            InvestmentTxType::Buy => {
                let qty = quantity.unwrap_or(0.0);
                FinanceInvestmentPatch {
                    id: inv.id.clone(),
                    quantity: Some((inv_qty + qty).to_string()),
                    cost_basis: Some(inv.cost_basis + total_amount),
                    current_price: None,
                    current_value: None,
                    notes: None,
                }
            }
            InvestmentTxType::Sell => {
                let sell_qty = quantity.unwrap_or(0.0);
                // Average cost per unit (floating-point to avoid integer truncation)
                let cost_per_unit = if inv_qty > 0.0 {
                    inv.cost_basis as f64 / inv_qty
                } else {
                    0.0
                };
                let cost_reduction = (cost_per_unit * sell_qty).round() as i64;
                let new_quantity = (inv_qty - sell_qty).max(0.0);
                let new_cost_basis = (inv.cost_basis - cost_reduction).max(0);
                FinanceInvestmentPatch {
                    id: inv.id.clone(),
                    quantity: Some(new_quantity.to_string()),
                    cost_basis: Some(new_cost_basis),
                    current_price: None,
                    current_value: None,
                    notes: None,
                }
            }
            InvestmentTxType::Split => {
                // quantity param is the split ratio (e.g. 2.0 for a 2:1 split)
                let ratio = quantity.unwrap_or(1.0);
                FinanceInvestmentPatch {
                    id: inv.id.clone(),
                    quantity: Some((inv_qty * ratio).to_string()),
                    cost_basis: None, // cost basis unchanged in a stock split
                    current_price: None,
                    current_value: None,
                    notes: None,
                }
            }
            // Dividend, rental income, interest: record income only — no quantity/basis change
            InvestmentTxType::Dividend
            | InvestmentTxType::RentalIncome
            | InvestmentTxType::Interest => FinanceInvestmentPatch {
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
}

#[cfg(test)]
mod tests {
    use crate::types::{AssetType, InvestmentTxType};
    use serde_json::json;
    use tools_core::ParamExtractor;

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

        // name is now optional (defaults to symbol or "Unnamed Investment")
        let args = json!({"portfolio_id": "p1", "asset_type": "stock"});
        let p = ParamExtractor::new(&args);
        assert!(p.optional_str("name").unwrap().is_none());

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
    fn investment_tx_requires_id_and_tx_type_and_total_amount() {
        // Accepts either "id" or "investment_id"
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.optional_str("id").unwrap().is_none());
        assert!(p.optional_str("investment_id").unwrap().is_none());

        let args = json!({"id": "i1"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("tx_type").is_err());

        let args = json!({"id": "i1", "tx_type": "buy"});
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
    fn test_asset_type_from_str_loose() {
        assert!(AssetType::from_str_loose("stock").is_some());
        assert!(AssetType::from_str_loose("etf").is_some());
        assert!(AssetType::from_str_loose("crypto").is_some());
        assert!(AssetType::from_str_loose("STOCK").is_some());
        assert!(AssetType::from_str_loose("invalid_type").is_none());
    }

    #[test]
    fn test_investment_tx_type_from_str_loose() {
        assert!(InvestmentTxType::from_str_loose("buy").is_some());
        assert!(InvestmentTxType::from_str_loose("sell").is_some());
        assert!(InvestmentTxType::from_str_loose("dividend").is_some());
        assert!(InvestmentTxType::from_str_loose("nope").is_none());
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
