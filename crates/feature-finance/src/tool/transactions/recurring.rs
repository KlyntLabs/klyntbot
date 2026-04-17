//! Recurring transaction handler for `FinanceTool`.

use jiff::{Timestamp, Zoned};
use serde_json::json;

use crate::currency::ensure_base_amount;
use crate::types::TransactionType;
use common::{Result, ToolError};
use storage::rows::finance::FinanceTransactionRow;
use tools_core::ParamExtractor;

use super::super::FinanceTool;

impl FinanceTool {
    pub(super) async fn tx_recurring_add(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let account_id = match p.optional_str("account_id")? {
            Some(id) => id.to_string(),
            None => {
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

        // Accept either "type" or "tx_type" — LLMs sometimes use the wrong one.
        let type_str = p
            .optional_str("type")?
            .or(p.optional_str("tx_type")?)
            .ok_or_else(|| {
                ToolError::InvalidParams("Missing required 'type' parameter".to_string())
            })?;
        let tx_type = TransactionType::from_str_loose(type_str).ok_or_else(|| {
            ToolError::InvalidParams(format!("Invalid transaction type: {}", type_str))
        })?;

        let amount = p.required_i64("amount")?;
        if amount <= 0 {
            return Err(ToolError::InvalidParams("Amount must be positive".to_string()).into());
        }

        let recurring_rule = p.required_str("recurring_rule")?;
        // Basic cron validation: must have exactly 5 whitespace-separated fields.
        let parts: Vec<&str> = recurring_rule.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(ToolError::InvalidParams(
                "Invalid recurring rule. Use cron format (e.g., '0 9 1 * *')".to_string(),
            )
            .into());
        }

        let category = p.optional_str("category")?;
        let counterparty = p.optional_str("counterparty")?;
        let notes = p.optional_str("notes")?;

        // Verify account exists and get its currency.
        let account_row = self
            .storage
            .accounts
            .get(account_id)
            .await?
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Account {} not found", account_id))
            })?;

        let now = Timestamp::now();
        let today = Zoned::now().date();

        let conv = ensure_base_amount(
            amount,
            &account_row.currency,
            &self.default_currency,
            &self.price_service,
        )
        .await?;

        let row = FinanceTransactionRow {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            tx_type: tx_type.as_str().to_string(),
            amount,
            currency: account_row.currency.clone(),
            category: category.map(|s| s.to_string()),
            subcategory: None,
            counterparty: counterparty.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
            tx_date: today.into(),
            transfer_id: None,
            is_recurring: true,
            recurring_rule: Some(recurring_rule.to_string()),
            created_at: now.into(),
            updated_at: now.into(),
            base_amount: conv.base_amount,
            base_currency: conv.base_currency,
            exchange_rate: conv.exchange_rate,
        };

        let inserted = self.storage.transactions.add(&row).await?;

        let result = json!({
            "tx_template": {
                "id": inserted.id,
                "account_id": inserted.account_id,
                "type": inserted.tx_type,
                "amount": inserted.amount,
                "currency": inserted.currency,
                "category": inserted.category,
                "is_recurring": inserted.is_recurring,
                "recurring_rule": inserted.recurring_rule,
            },
            "recurring_rule": recurring_rule,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }
}
