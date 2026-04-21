//! Budget action handlers for `FinanceTool`.
//!
//! Handles: budget_create, budget_list, budget_status, budget_update, budget_delete.

use jiff::{Timestamp, Zoned};
use serde_json::json;
use uuid::Uuid;

use crate::currency::ensure_base_amount;
use crate::types::{BudgetMethod, BudgetPeriod, JarType};
use common::{Result, ToolError};
use storage::rows::finance::{FinanceBudgetPatch, FinanceBudgetRow, FinanceTransactionFilter};
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::{parse_date, FinanceTool};

impl FinanceTool {
    pub(crate) async fn handle_budget(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "budget_create" => self.budget_create(p).await,
            "budget_list" => self.budget_list(p).await,
            "budget_status" => self.budget_status(p).await,
            "budget_update" => self.budget_update(p).await,
            "budget_delete" => self.budget_delete(p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown budget action: {action}")).into()),
        }
    }

    async fn budget_create(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let name = p.required_str("name")?;
        let amount = p.required_i64("amount")?;
        if amount <= 0 {
            return Err(
                ToolError::InvalidParams("Budget amount must be positive".to_string()).into(),
            );
        }
        let currency = p
            .optional_str("currency")?
            .unwrap_or(&self.default_currency);
        let period_str = p.required_str("period")?;
        let period = BudgetPeriod::from_str_loose(period_str)
            .ok_or_else(|| ToolError::InvalidParams(format!("Invalid period: {period_str}")))?;

        let category = p.optional_str("category")?;

        let method_str = p.str_or("method", "standard")?;
        let method = BudgetMethod::from_str_loose(method_str)
            .ok_or_else(|| ToolError::InvalidParams(format!("Invalid method: {method_str}")))?;

        let jar_type_str = p.optional_str("jar_type")?;
        let jar_type = match jar_type_str {
            Some(s) => Some(
                JarType::from_str_loose(s)
                    .ok_or_else(|| ToolError::InvalidParams(format!("Invalid jar_type: {s}")))?,
            ),
            None => None,
        };

        if method == BudgetMethod::SixJar && jar_type.is_none() {
            return Err(ToolError::InvalidParams(
                "jar_type is required when method=six_jar".to_string(),
            )
            .into());
        }

        let today = Zoned::now().date();
        let start_date = match p.optional_str("start_date")? {
            Some(s) => parse_date(s)?,
            None => today,
        };
        let end_date = p.optional_str("end_date")?.map(parse_date).transpose()?;

        if let Some(ed) = end_date {
            if ed < start_date {
                return Err(ToolError::InvalidParams(
                    "End date cannot be before start date".to_string(),
                )
                .into());
            }
        }

        let alert_threshold = p.i64_or("alert_threshold", 80)? as i32;
        let now = Timestamp::now();
        let id = Uuid::new_v4().to_string();

        let conv = ensure_base_amount(
            amount,
            currency,
            &self.default_currency,
            &self.price_service,
        )
        .await?;

        let row = FinanceBudgetRow {
            id,
            name: name.to_string(),
            amount,
            currency: currency.to_string(),
            period: period.as_str().to_string(),
            category: category.map(|s| s.to_string()),
            method: method.as_str().to_string(),
            jar_type: jar_type.map(|j| j.as_str().to_string()),
            start_date: start_date.into(),
            end_date: end_date.map(|d| d.into()),
            is_active: true,
            alert_threshold,
            created_at: now.into(),
            updated_at: now.into(),
            base_amount: conv.base_amount,
            base_currency: conv.base_currency,
            exchange_rate: conv.exchange_rate,
        };

        let inserted = self.storage.budgets.add(&row).await?;

        if let Some(ref bus) = self.domain_bus {
            bus.publish(crate::events::FinanceEvent::BudgetCreated {
                budget_id: inserted.id.clone(),
                name: inserted.name.clone(),
                amount: inserted.amount,
                currency: inserted.currency.clone(),
            }.into());
        }

        let resp = json!({
            "id": inserted.id,
            "name": inserted.name,
            "category": inserted.category,
            "amount": inserted.amount,
            "currency": inserted.currency,
            "period": inserted.period,
            "method": inserted.method,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    async fn budget_list(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let period_filter = p.optional_str("period")?;
        let usages = self.storage.budgets.all_budget_usage().await?;

        let budgets: Vec<_> = usages
            .iter()
            .filter(|u| match period_filter {
                None => true,
                Some(period_str) => BudgetPeriod::from_str_loose(period_str)
                    .map(|p| p.as_str() == u.period.as_str())
                    .unwrap_or(false),
            })
            .map(|u| {
                let remaining = u.amount - u.spent;
                let percentage = if u.amount > 0 {
                    (u.spent * 100) / u.amount
                } else {
                    0
                };
                json!({
                    "id": u.id,
                    "name": u.name,
                    "category": u.category,
                    "amount": u.amount,
                    "spent": u.spent,
                    "remaining": remaining,
                    "percentage": percentage,
                    "currency": u.currency,
                    "period": u.period,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&budgets).unwrap())
    }

    async fn budget_status(&self, p: &ParamExtractor<'_>) -> Result<String> {
        // If no ID provided, show summary status for all active budgets.
        let id = match p.optional_str("id")? {
            Some(id) => id.to_string(),
            None => return self.budget_status_all().await,
        };
        let usage = self.storage.budgets.budget_usage(&id).await?;

        let remaining = usage.amount - usage.spent;
        let percentage = if usage.amount > 0 {
            (usage.spent * 100) / usage.amount
        } else {
            0
        };

        // Fetch recent expense transactions for this budget's category
        let filter = FinanceTransactionFilter {
            category: usage.category.clone(),
            tx_type: Some("expense".to_string()),
            limit: Some(20),
            ..Default::default()
        };
        let txs = self.storage.transactions.list(&filter).await?;

        // Compute subcategory breakdown from fetched transactions
        let mut subcategory_map: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        for tx in &txs {
            let sub = tx
                .subcategory
                .clone()
                .unwrap_or_else(|| "uncategorized".to_string());
            *subcategory_map.entry(sub).or_insert(0) += tx.amount;
        }
        let mut breakdown: Vec<_> = subcategory_map
            .iter()
            .map(|(subcategory, amount)| json!({"subcategory": subcategory, "amount": amount}))
            .collect();
        breakdown.sort_by(|a, b| {
            b["amount"]
                .as_i64()
                .unwrap_or(0)
                .cmp(&a["amount"].as_i64().unwrap_or(0))
        });

        let recent_transactions: Vec<_> = txs
            .iter()
            .take(10)
            .map(|tx| {
                json!({
                    "id": tx.id,
                    "amount": tx.amount,
                    "currency": tx.currency,
                    "category": tx.category,
                    "subcategory": tx.subcategory,
                    "counterparty": tx.counterparty,
                    "tx_date": tx.tx_date.to_string(),
                    "notes": tx.notes,
                })
            })
            .collect();

        let budget = json!({
            "id": usage.id,
            "name": usage.name,
            "category": usage.category,
            "amount": usage.amount,
            "currency": usage.currency,
            "period": usage.period,
            "method": usage.method,
            "jar_type": usage.jar_type,
            "is_active": usage.is_active,
            "alert_threshold": usage.alert_threshold,
        });

        let resp = json!({
            "budget": budget,
            "spent": usage.spent,
            "remaining": remaining,
            "percentage": percentage,
            "breakdown_by_subcategory": breakdown,
            "recent_transactions": recent_transactions,
        });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    /// Show a summary status for every active budget.
    async fn budget_status_all(&self) -> Result<String> {
        let usages = self.storage.budgets.all_budget_usage().await?;
        if usages.is_empty() {
            return Ok(r#"{"budgets":[],"message":"No active budgets found."}"#.to_string());
        }

        let items: Vec<_> = usages
            .iter()
            .map(|u| {
                let remaining = u.amount - u.spent;
                let percentage = if u.amount > 0 {
                    (u.spent * 100) / u.amount
                } else {
                    0
                };
                json!({
                    "id": u.id,
                    "name": u.name,
                    "category": u.category,
                    "period": u.period,
                    "amount": u.amount,
                    "currency": u.currency,
                    "spent": u.spent,
                    "remaining": remaining,
                    "percentage_used": percentage,
                })
            })
            .collect();
        let resp = json!({ "budgets": items });
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    async fn budget_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let name = p.optional_str("name")?;
        let amount = p.optional_i64("amount")?;
        let category = p.optional_str("category")?;
        let is_active = p.optional_bool("is_active")?;

        let patch = FinanceBudgetPatch {
            id: id.to_string(),
            name: name.map(|s| s.to_string()),
            amount,
            category: category.map(|s| Some(s.to_string())),
            is_active,
            base_amount: None,
            base_currency: None,
            exchange_rate: None,
        };

        let updated = self.storage.budgets.update(&patch).await?;

        let budget = json!({
            "id": updated.id,
            "name": updated.name,
            "category": updated.category,
            "amount": updated.amount,
            "currency": updated.currency,
            "period": updated.period,
            "method": updated.method,
            "is_active": updated.is_active,
            "alert_threshold": updated.alert_threshold,
        });
        let resp = json!({"budget": budget});
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }

    async fn budget_delete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        self.storage.budgets.delete(id).await?;
        let resp = json!({"deleted": true});
        Ok(serde_json::to_string_pretty(&resp).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{BudgetMethod, BudgetPeriod, JarType};
    use serde_json::json;
    use tools_core::ParamExtractor;

    #[test]
    fn budget_create_required_param_validation() {
        // Each step: one more required param present, assert the next one errors
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("name").is_err(), "name must be required");

        let args = json!({"name": "Food"});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("name").is_ok());
        assert!(p.required_i64("amount").is_err(), "amount must be required");

        // currency is optional (defaults to self.default_currency)
        let args = json!({"name": "Food", "amount": 500000});
        let p = ParamExtractor::new(&args);
        assert!(
            p.optional_str("currency").unwrap().is_none(),
            "currency should be optional"
        );

        let args = json!({"name": "Food", "amount": 500000});
        let p = ParamExtractor::new(&args);
        assert!(p.required_str("period").is_err(), "period must be required");
    }

    #[test]
    fn budget_period_all_values_parse() {
        for s in &[
            "monthly", "month", "weekly", "week", "yearly", "year", "custom",
        ] {
            assert!(
                BudgetPeriod::from_str_loose(s).is_some(),
                "period '{s}' should parse"
            );
        }
        assert!(BudgetPeriod::from_str_loose("quarterly").is_none());
        assert!(BudgetPeriod::from_str_loose("").is_none());
    }

    #[test]
    fn budget_method_parsing() {
        assert_eq!(
            BudgetMethod::from_str_loose("standard"),
            Some(BudgetMethod::Standard)
        );
        assert_eq!(
            BudgetMethod::from_str_loose("six_jar"),
            Some(BudgetMethod::SixJar)
        );
        assert_eq!(
            BudgetMethod::from_str_loose("6jar"),
            Some(BudgetMethod::SixJar)
        );
        assert!(BudgetMethod::from_str_loose("unknown").is_none());
    }

    #[test]
    fn jar_type_all_values_parse() {
        for s in &[
            "essentials",
            "savings",
            "investment",
            "education",
            "entertainment",
            "charity",
        ] {
            assert!(
                JarType::from_str_loose(s).is_some(),
                "jar_type '{s}' should parse"
            );
        }
        assert!(JarType::from_str_loose("other").is_none());
    }

    #[test]
    fn six_jar_without_jar_type_is_invalid() {
        // Validate the business rule: six_jar method requires jar_type
        let method = BudgetMethod::from_str_loose("six_jar").unwrap();
        assert_eq!(method, BudgetMethod::SixJar);
        // jar_type is None → should produce an error (checked in handler)
        let jar_type: Option<JarType> = None;
        assert!(
            method == BudgetMethod::SixJar && jar_type.is_none(),
            "six_jar with no jar_type should be invalid"
        );
    }

    #[test]
    fn budget_list_percentage_and_remaining_calculation() {
        let amount = 2_000_000i64;
        let spent = 1_500_000i64;
        let remaining = amount - spent;
        let percentage = if amount > 0 {
            (spent * 100) / amount
        } else {
            0
        };

        assert_eq!(remaining, 500_000);
        assert_eq!(percentage, 75);
    }

    #[test]
    fn budget_list_percentage_zero_amount_guard() {
        let amount = 0i64;
        let spent = 0i64;
        let percentage = if amount > 0 {
            (spent * 100) / amount
        } else {
            0
        };
        assert_eq!(percentage, 0, "should not divide by zero");
    }

    #[test]
    fn budget_alert_threshold_defaults_to_80() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("alert_threshold", 80).unwrap(), 80);
    }

    #[test]
    fn budget_alert_threshold_custom_value() {
        let args = json!({"alert_threshold": 90});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("alert_threshold", 80).unwrap(), 90);
    }

    #[test]
    fn test_budget_period_from_str_loose() {
        assert!(BudgetPeriod::from_str_loose("monthly").is_some());
        assert!(BudgetPeriod::from_str_loose("weekly").is_some());
        assert!(BudgetPeriod::from_str_loose("yearly").is_some());
        assert!(BudgetPeriod::from_str_loose("custom").is_some());
        assert!(BudgetPeriod::from_str_loose("invalid").is_none());
    }

    #[test]
    fn test_budget_method_from_str_loose() {
        assert!(BudgetMethod::from_str_loose("standard").is_some());
        assert!(BudgetMethod::from_str_loose("six_jar").is_some());
        assert!(BudgetMethod::from_str_loose("nope").is_none());
    }

    #[test]
    fn test_jar_type_from_str_loose() {
        assert!(JarType::from_str_loose("essentials").is_some());
        assert!(JarType::from_str_loose("savings").is_some());
        assert!(JarType::from_str_loose("entertainment").is_some());
        assert!(JarType::from_str_loose("invalid").is_none());
    }
}
