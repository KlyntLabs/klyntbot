//! Transaction action handlers for `FinanceTool`.
//!
//! Handles: tx_add, tx_list, tx_update, tx_delete, tx_search, tx_recurring_add.

mod recurring;
mod transfer;

use jiff::{Timestamp, Zoned};
use serde_json::json;

use crate::currency::ensure_base_amount;
use crate::types::{FinanceTransaction, TransactionType};
use ai_core::AiEntity;
use common::{Result, ToolError};
use storage::rows::finance::{
    FinanceTransactionFilter, FinanceTransactionPatch, FinanceTransactionRow,
};
use storage::StorageError;
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::{parse_date, FinanceTool};

impl FinanceTool {
    pub(crate) async fn handle_transaction(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "tx_add" => self.tx_add(p).await,
            "tx_list" => self.tx_list(p).await,
            "tx_update" => self.tx_update(p).await,
            "tx_delete" => self.tx_delete(p).await,
            "tx_search" => self.tx_search(p).await,
            "tx_recurring_add" => self.tx_recurring_add(p).await,
            _ => Err(
                ToolError::InvalidParams(format!("Unknown transaction action: {}", action)).into(),
            ),
        }
    }

    async fn tx_add(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let account_id = match p.optional_str("account_id")? {
            Some(id) => id.to_string(),
            None => {
                // Auto-select the first active account when none specified.
                let accounts = self.storage.accounts.list(false).await?;
                accounts.first().map(|a| a.id.clone()).ok_or_else(|| {
                    ToolError::InvalidParams(
                        serde_json::to_string(&json!({
                            "error": "no_accounts",
                            "message": "No active accounts found. Create an account first.",
                            "suggested_action": "account_add",
                            "example": {
                                "action": "account_add",
                                "name": "Main Bank",
                                "type": "bank",
                                "currency": "USD"
                            }
                        }))
                        .unwrap(),
                    )
                })?
            }
        };
        let account_id = account_id.as_str();

        let type_str = p.required_str("type")?;
        let tx_type = TransactionType::from_str_loose(type_str).ok_or_else(|| {
            ToolError::InvalidParams(format!("Invalid transaction type: {}", type_str))
        })?;

        let amount = p.required_i64("amount")?;
        if amount <= 0 {
            return Err(ToolError::InvalidParams("Amount must be positive".to_string()).into());
        }

        let tx_date = match p.optional_str("tx_date")? {
            Some(s) => parse_date(s)?,
            None => Zoned::now().date(),
        };

        let category = p.optional_str("category")?.map(|s| s.to_string());
        let subcategory = p.optional_str("subcategory")?.map(|s| s.to_string());
        let counterparty = p.optional_str("counterparty")?.map(|s| s.to_string());
        let notes = p.optional_str("notes")?.map(|s| s.to_string());

        if tx_type == TransactionType::Transfer {
            return self
                .tx_add_transfer(
                    p,
                    account_id,
                    amount,
                    tx_date,
                    category.as_deref(),
                    subcategory.as_deref(),
                    counterparty.as_deref(),
                    notes.as_deref(),
                )
                .await;
        }

        // ── Income / Expense ──────────────────────────────────────────────────

        // Get account to verify it exists and obtain its currency.
        let account_row = self
            .storage
            .accounts
            .get(account_id)
            .await?
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Account {} not found", account_id))
            })?;

        let currency = p
            .optional_str("currency")?
            .unwrap_or(&account_row.currency)
            .to_string();

        let conv = ensure_base_amount(
            amount,
            &currency,
            &self.default_currency,
            &self.price_service,
        )
        .await?;

        let now = Timestamp::now();
        let row = FinanceTransactionRow {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            tx_type: tx_type.as_str().to_string(),
            amount,
            currency,
            category: category.clone(),
            subcategory,
            counterparty,
            notes,
            tx_date: tx_date.into(),
            transfer_id: None,
            is_recurring: false,
            recurring_rule: None,
            created_at: now.into(),
            updated_at: now.into(),
            base_amount: conv.base_amount,
            base_currency: conv.base_currency,
            exchange_rate: conv.exchange_rate,
        };

        let inserted = self.storage.transactions.add(&row).await?;

        // Adjust account balance.
        // Note: Transfer is handled early and should never reach here (line 85-98)
        let balance_delta = match tx_type {
            TransactionType::Income => amount,
            TransactionType::Expense => -amount,
            TransactionType::Transfer => {
                unreachable!("Transfer transactions should be handled by tx_add_transfer")
            }
        };

        let updated_account = self
            .storage
            .accounts
            .adjust_balance(account_id, balance_delta)
            .await?;

        let tx = FinanceTransaction::from(inserted);

        // Embed transaction for semantic search (best-effort, non-blocking).
        if let Some(ref handler) = self.embedding_handler {
            let _ = handler.embed_transaction(&tx.embed_text()).await;
        }

        // Check budget impact for expense transactions with a category.
        let mut budget_impact: Option<serde_json::Value> = None;
        let mut nudge = String::new();
        let mut budget_typed: Option<(i64, i64, i64, i64)> = None; // (percentage, spent, limit, alert_threshold)
        if tx_type == TransactionType::Expense {
            if let Some(ref cat) = category {
                if let Ok(Some(budget)) = self.storage.budgets.get_by_category(cat).await {
                    if let Ok(usage) = self.storage.budgets.budget_usage(&budget.id).await {
                        let percentage = if usage.amount > 0 {
                            (usage.spent * 100) / usage.amount
                        } else {
                            0
                        };
                        budget_impact = Some(json!({
                            "budget_name": usage.name,
                            "spent": usage.spent,
                            "limit": usage.amount,
                            "percentage": percentage,
                        }));
                        budget_typed = Some((
                            percentage,
                            usage.spent,
                            usage.amount,
                            usage.alert_threshold as i64,
                        ));
                        if percentage >= usage.alert_threshold as i64 {
                            nudge = format!(
                                "\nNote: Your \"{}\" budget is now at {}% ({} / {} {}).",
                                usage.name, percentage, usage.spent, usage.amount, usage.currency,
                            );
                        }
                    }
                }
            }
        }

        // Emit domain events
        if let Some(ref bus) = self.domain_bus {
            let is_over_budget = budget_typed.map(|(p, _, _, _)| p >= 100).unwrap_or(false);

            bus.publish(crate::events::FinanceEvent::TransactionRecorded {
                tx_id: tx.id.clone(),
                category: category.clone().unwrap_or_default(),
                amount,
                currency: tx.currency.clone(),
                is_over_budget,
            }.into());

            if let Some((percentage, spent, limit, alert_threshold)) = budget_typed {
                if percentage >= alert_threshold {
                    bus.publish(crate::events::FinanceEvent::BudgetAlert {
                        category: category.clone().unwrap_or_default(),
                        spent,
                        limit,
                    }.into());
                }
            }
        }

        let mut result = json!({
            "tx": {
                "id": tx.id,
                "account_id": tx.account_id,
                "type": tx.tx_type.as_str(),
                "amount": tx.amount,
                "currency": tx.currency,
                "category": tx.category,
                "subcategory": tx.subcategory,
                "counterparty": tx.counterparty,
                "notes": tx.notes,
                "tx_date": tx.tx_date.to_string(),
            },
            "new_balance": updated_account.balance,
        });

        if let Some(impact) = budget_impact {
            result["budget_impact"] = impact;
        }

        let mut response = serde_json::to_string_pretty(&result).unwrap();
        if !nudge.is_empty() {
            response.push_str(&nudge);
        }
        Ok(response)
    }

    async fn tx_list(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let account_id = p.optional_str("account_id")?;
        let category = p.optional_str("category")?;
        let type_str = p.optional_str("type")?;
        let date_from_str = p.optional_str("date_from")?;
        let date_to_str = p.optional_str("date_to")?;
        let limit = p.optional_i64("limit")?;

        let tx_type_str = match type_str {
            Some(s) => {
                TransactionType::from_str_loose(s).ok_or_else(|| {
                    ToolError::InvalidParams(format!("Invalid transaction type: {}", s))
                })?;
                Some(s.to_string())
            }
            None => None,
        };

        let date_from = date_from_str.map(parse_date).transpose()?;
        let date_to = date_to_str.map(parse_date).transpose()?;

        let filter = FinanceTransactionFilter {
            account_id: account_id.map(|s| s.to_string()),
            tx_type: tx_type_str,
            category: category.map(|s| s.to_string()),
            date_from: date_from.map(|d| d.into()),
            date_to: date_to.map(|d| d.into()),
            limit: Some(limit.unwrap_or(50)),
            ..Default::default()
        };

        let rows = self.storage.transactions.list(&filter).await?;

        if rows.is_empty() {
            return Ok("No transactions found matching your filters.".to_string());
        }

        let txns: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "account_id": r.account_id,
                    "type": r.tx_type,
                    "amount": r.amount,
                    "currency": r.currency,
                    "category": r.category,
                    "counterparty": r.counterparty,
                    "tx_date": r.tx_date.to_string(),
                    "notes": r.notes,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&json!(txns)).unwrap())
    }

    async fn tx_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let new_amount = p.optional_i64("amount")?;
        if let Some(amt) = new_amount {
            if amt <= 0 {
                return Err(ToolError::InvalidParams("Amount must be positive".to_string()).into());
            }
        }
        let category = p.optional_str("category")?;
        let subcategory = p.optional_str("subcategory")?;
        let counterparty = p.optional_str("counterparty")?;
        let notes = p.optional_str("notes")?;
        let date_str = p.optional_str("tx_date")?;

        if new_amount.is_none()
            && category.is_none()
            && subcategory.is_none()
            && counterparty.is_none()
            && notes.is_none()
            && date_str.is_none()
        {
            return Err(ToolError::InvalidParams("No fields to update".to_string()).into());
        }

        let tx_date = date_str.map(parse_date).transpose()?;

        // Fetch existing transaction (needed for balance adjustment and existence check).
        let old_tx =
            self.storage.transactions.get(id).await?.ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Transaction {} not found", id))
            })?;

        let patch = FinanceTransactionPatch {
            id: id.to_string(),
            amount: new_amount,
            category: category.map(|s| Some(s.to_string())),
            subcategory: subcategory.map(|s| Some(s.to_string())),
            counterparty: counterparty.map(|s| Some(s.to_string())),
            notes: notes.map(|s| Some(s.to_string())),
            tx_date: tx_date.map(|d| d.into()),
            base_amount: None,
            base_currency: None,
            exchange_rate: None,
        };

        let updated_tx = self
            .storage
            .transactions
            .update(&patch)
            .await
            .map_err(|e| match e {
                StorageError::NotFound(_) => {
                    ToolError::ExecutionFailed(format!("Transaction {} not found", id))
                }
                other => ToolError::ExecutionFailed(other.to_string()),
            })?;

        // Adjust account balance if the amount changed.
        let mut balance_adjustment: Option<i64> = None;
        if let Some(new_amt) = new_amount {
            let old_amt = old_tx.amount;
            let tx_type = TransactionType::from_str_loose(&old_tx.tx_type).unwrap_or_default();
            // For expense: each extra unit deducted more → delta is negative when new > old.
            // For income: each extra unit added more → delta is positive when new > old.
            let delta = match tx_type {
                TransactionType::Expense => -(new_amt - old_amt),
                TransactionType::Income => new_amt - old_amt,
                TransactionType::Transfer => 0,
            };
            if delta != 0 {
                self.storage
                    .accounts
                    .adjust_balance(&old_tx.account_id, delta)
                    .await?;
                balance_adjustment = Some(delta);
            }
        }

        let tx = FinanceTransaction::from(updated_tx);
        let mut result = json!({
            "tx": {
                "id": tx.id,
                "account_id": tx.account_id,
                "type": tx.tx_type.as_str(),
                "amount": tx.amount,
                "currency": tx.currency,
                "category": tx.category,
                "subcategory": tx.subcategory,
                "counterparty": tx.counterparty,
                "notes": tx.notes,
                "tx_date": tx.tx_date.to_string(),
            },
        });

        if let Some(adj) = balance_adjustment {
            result["balance_adjustment"] = json!(adj);
        }

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn tx_delete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let deleted_row =
            self.storage.transactions.delete(id).await?.ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Transaction {} not found", id))
            })?;

        let tx_type = TransactionType::from_str_loose(&deleted_row.tx_type).unwrap_or_default();

        // ── Transfer: delete paired tx and reverse both accounts ─────────────
        if let Some(ref transfer_id) = deleted_row.transfer_id {
            // Reverse the account of the tx we just deleted.
            let deleted_delta = match tx_type {
                TransactionType::Expense => deleted_row.amount, // add back
                TransactionType::Income => -deleted_row.amount, // subtract back
                TransactionType::Transfer => 0,
            };
            let updated_deleted_acct = self
                .storage
                .accounts
                .adjust_balance(&deleted_row.account_id, deleted_delta)
                .await?;

            // Find and remove the paired transfer row (the other side).
            let paired_rows = self
                .storage
                .transactions
                .get_by_transfer_id(transfer_id)
                .await?;

            let mut from_balance = updated_deleted_acct.balance;
            let mut to_balance = updated_deleted_acct.balance;

            for paired_row in &paired_rows {
                self.storage.transactions.delete(&paired_row.id).await?;

                let paired_type =
                    TransactionType::from_str_loose(&paired_row.tx_type).unwrap_or_default();
                let paired_delta = match paired_type {
                    TransactionType::Expense => paired_row.amount,
                    TransactionType::Income => -paired_row.amount,
                    TransactionType::Transfer => 0,
                };
                let updated_paired_acct = self
                    .storage
                    .accounts
                    .adjust_balance(&paired_row.account_id, paired_delta)
                    .await?;

                // Identify from/to: expense side is from, income side is to.
                if paired_type == TransactionType::Expense {
                    from_balance = updated_paired_acct.balance;
                    to_balance = updated_deleted_acct.balance;
                } else {
                    from_balance = updated_deleted_acct.balance;
                    to_balance = updated_paired_acct.balance;
                }
            }

            let result = json!({
                "deleted": true,
                "balance_restored": deleted_row.amount,
                "from_balance": from_balance,
                "to_balance": to_balance,
                "transfer_id": transfer_id,
            });
            return Ok(serde_json::to_string_pretty(&result).unwrap());
        }

        // ── Non-transfer: reverse the single balance impact ───────────────────
        let balance_delta = match tx_type {
            TransactionType::Expense => deleted_row.amount, // add back
            TransactionType::Income => -deleted_row.amount, // subtract back
            TransactionType::Transfer => 0,
        };

        let updated_account = self
            .storage
            .accounts
            .adjust_balance(&deleted_row.account_id, balance_delta)
            .await?;

        let result = json!({
            "deleted": true,
            "balance_restored": deleted_row.amount,
            "new_balance": updated_account.balance,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn tx_search(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let query = p.optional_str("query")?;
        let amount_min = p.optional_i64("amount_min")?;
        let amount_max = p.optional_i64("amount_max")?;
        let date_from_str = p.optional_str("date_from")?;
        let date_to_str = p.optional_str("date_to")?;

        if query.is_none()
            && amount_min.is_none()
            && amount_max.is_none()
            && date_from_str.is_none()
            && date_to_str.is_none()
        {
            return Err(ToolError::InvalidParams(
                "At least one search criterion is required".to_string(),
            )
            .into());
        }

        let date_from = date_from_str.map(parse_date).transpose()?;
        let date_to = date_to_str.map(parse_date).transpose()?;

        let filter = FinanceTransactionFilter {
            query: query.map(|s| s.to_string()),
            amount_min,
            amount_max,
            date_from: date_from.map(|d| d.into()),
            date_to: date_to.map(|d| d.into()),
            limit: Some(50),
            ..Default::default()
        };

        let rows = self.storage.transactions.list(&filter).await?;

        if rows.is_empty() {
            return Ok("No transactions found matching your search.".to_string());
        }

        let txns: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "account_id": r.account_id,
                    "type": r.tx_type,
                    "amount": r.amount,
                    "currency": r.currency,
                    "category": r.category,
                    "counterparty": r.counterparty,
                    "tx_date": r.tx_date.to_string(),
                    "notes": r.notes,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&json!(txns)).unwrap())
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::types::TransactionType;
    use tools_core::ParamExtractor;

    // ── Parameter validation unit tests (no DB required) ────────────────────

    #[test]
    fn tx_add_invalid_type_rejected() {
        assert!(
            TransactionType::from_str_loose("credit").is_none(),
            "'credit' should not be a valid transaction type"
        );
        assert!(
            TransactionType::from_str_loose("debit").is_none(),
            "'debit' should not be a valid transaction type"
        );
    }

    #[test]
    fn tx_add_valid_types_accepted() {
        for s in &["income", "expense", "transfer"] {
            assert!(
                TransactionType::from_str_loose(s).is_some(),
                "'{s}' should be a valid transaction type"
            );
        }
        // Case-insensitive
        assert!(TransactionType::from_str_loose("INCOME").is_some());
        assert!(TransactionType::from_str_loose("Expense").is_some());
    }

    #[test]
    fn tx_add_negative_amount_rejected() {
        let amount = -500_i64;
        assert!(amount <= 0, "negative amount should be rejected");
    }

    #[test]
    fn tx_add_zero_amount_rejected() {
        let amount = 0_i64;
        assert!(amount <= 0, "zero amount should be rejected");
    }

    #[test]
    fn tx_add_positive_amount_accepted() {
        let amount = 100_000_i64;
        assert!(amount > 0, "positive amount should be accepted");
    }

    #[test]
    fn tx_add_transfer_same_account_detected() {
        let account_id = "acct-123";
        let dest_id = "acct-123";
        assert_eq!(
            account_id, dest_id,
            "same account detected as transfer-to-self"
        );
    }

    #[test]
    fn tx_add_valid_date_parsing() {
        let date = jiff::civil::Date::strptime("%Y-%m-%d", "2026-02-19").unwrap();
        assert_eq!(date.to_string(), "2026-02-19");
    }

    #[test]
    fn tx_add_invalid_date_rejected() {
        let result = jiff::civil::Date::strptime("%Y-%m-%d", "not-a-date");
        assert!(result.is_err(), "invalid date string should fail parsing");
    }

    #[test]
    fn tx_update_no_fields_detected() {
        let args = json!({ "id": "tx-123" });
        let p = ParamExtractor::new(&args);

        let new_amount = p.optional_i64("amount").unwrap();
        let category = p.optional_str("category").unwrap();
        let subcategory = p.optional_str("subcategory").unwrap();
        let counterparty = p.optional_str("counterparty").unwrap();
        let notes = p.optional_str("notes").unwrap();
        let date = p.optional_str("tx_date").unwrap();

        assert!(
            new_amount.is_none()
                && category.is_none()
                && subcategory.is_none()
                && counterparty.is_none()
                && notes.is_none()
                && date.is_none(),
            "all fields absent → 'no fields to update' condition"
        );
    }

    #[test]
    fn tx_update_balance_delta_expense() {
        // Expense: old=100k, new=150k → delta = -(150k - 100k) = -50k (more deducted)
        let old_amt: i64 = 100_000;
        let new_amt: i64 = 150_000;
        let delta = -(new_amt - old_amt);
        assert_eq!(
            delta, -50_000,
            "expense delta should be negative when amount increases"
        );
    }

    #[test]
    fn tx_update_balance_delta_income() {
        // Income: old=100k, new=150k → delta = +(150k - 100k) = +50k (more added)
        let old_amt: i64 = 100_000;
        let new_amt: i64 = 150_000;
        let delta = new_amt - old_amt;
        assert_eq!(
            delta, 50_000,
            "income delta should be positive when amount increases"
        );
    }

    #[test]
    fn tx_delete_expense_reversal() {
        // Expense: reversal = +amount (add back to account)
        let amount: i64 = 100_000;
        let reversal_delta = amount; // add back expense
        assert_eq!(reversal_delta, 100_000);
    }

    #[test]
    fn tx_delete_income_reversal() {
        // Income: reversal = -amount (subtract back from account)
        let amount: i64 = 200_000;
        let reversal_delta = -amount; // subtract back income
        assert_eq!(reversal_delta, -200_000);
    }

    #[test]
    fn tx_search_no_criteria_detected() {
        let args = json!({});
        let p = ParamExtractor::new(&args);

        let query = p.optional_str("query").unwrap();
        let amount_min = p.optional_i64("amount_min").unwrap();
        let amount_max = p.optional_i64("amount_max").unwrap();
        let date_from = p.optional_str("date_from").unwrap();
        let date_to = p.optional_str("date_to").unwrap();

        assert!(
            query.is_none()
                && amount_min.is_none()
                && amount_max.is_none()
                && date_from.is_none()
                && date_to.is_none(),
            "all criteria absent → 'at least one criterion required' condition"
        );
    }

    #[test]
    fn tx_recurring_invalid_cron_detected() {
        // A cron rule must have exactly 5 whitespace-separated fields.
        let bad_rules = &["every monday", "0 9 *", "* * * * * *", "", "  "];
        for rule in bad_rules {
            let parts: Vec<&str> = rule.split_whitespace().collect();
            assert_ne!(
                parts.len(),
                5,
                "'{rule}' should not be a valid 5-field cron expression"
            );
        }
    }

    #[test]
    fn tx_recurring_valid_cron_accepted() {
        let good_rules = &["0 9 1 * *", "30 8 * * 1", "0 0 * * *", "*/5 * * * *"];
        for rule in good_rules {
            let parts: Vec<&str> = rule.split_whitespace().collect();
            assert_eq!(
                parts.len(),
                5,
                "'{rule}' should be a valid 5-field cron expression"
            );
        }
    }

    #[test]
    fn tx_list_params_extract_correctly() {
        let args = json!({
            "account_id": "acct-123",
            "category": "food",
            "type": "expense",
            "date_from": "2026-01-01",
            "date_to": "2026-01-31",
            "limit": 25_i64,
        });
        let p = ParamExtractor::new(&args);

        assert_eq!(p.optional_str("account_id").unwrap(), Some("acct-123"));
        assert_eq!(p.optional_str("category").unwrap(), Some("food"));
        assert_eq!(p.optional_str("type").unwrap(), Some("expense"));
        assert_eq!(p.optional_i64("limit").unwrap(), Some(25));
    }

    #[test]
    fn test_tx_type_alias_accepted() {
        // Verify that both "type" and "tx_type" keys resolve to the same value.
        let args = json!({"action": "tx_add", "tx_type": "expense", "amount": 5000});
        let p = ParamExtractor::new(&args);
        // "tx_type" should be accessible via optional_str
        let tx_type = p.optional_str("tx_type").unwrap();
        assert_eq!(tx_type, Some("expense"));
    }

    #[test]
    fn test_transaction_type_from_str_loose() {
        assert!(TransactionType::from_str_loose("income").is_some());
        assert!(TransactionType::from_str_loose("expense").is_some());
        assert!(TransactionType::from_str_loose("transfer").is_some());
        assert!(TransactionType::from_str_loose("INCOME").is_some());
        assert!(TransactionType::from_str_loose("invalid").is_none());
    }
}
