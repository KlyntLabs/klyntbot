//! Transfer transaction handler for `FinanceTool`.

use chrono::{NaiveDate, Utc};
use serde_json::json;

use crate::types::TransactionType;
use common::{Result, ToolError};
use storage::rows::finance::FinanceTransactionRow;
use tools_core::ParamExtractor;

use super::super::FinanceTool;

impl FinanceTool {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn tx_add_transfer(
        &self,
        p: &ParamExtractor<'_>,
        account_id: &str,
        amount: i64,
        tx_date: NaiveDate,
        category: Option<&str>,
        subcategory: Option<&str>,
        counterparty: Option<&str>,
        notes: Option<&str>,
    ) -> Result<String> {
        let dest_id = p.optional_str("transfer_to_account_id")?.ok_or_else(|| {
            ToolError::InvalidParams(
                "transfer_to_account_id is required for transfer transactions".to_string(),
            )
        })?;

        if account_id == dest_id {
            return Err(ToolError::InvalidParams(
                "Cannot transfer to the same account".to_string(),
            )
            .into());
        }

        let src_row = self
            .storage
            .accounts
            .get(account_id)
            .await?
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Account {} not found", account_id))
            })?;

        let dst_row =
            self.storage.accounts.get(dest_id).await?.ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Account {} not found", dest_id))
            })?;

        if src_row.currency != dst_row.currency {
            return Err(ToolError::ExecutionFailed(
                "Cross-currency transfers not yet supported".to_string(),
            )
            .into());
        }

        let currency = src_row.currency.clone();
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let from_row = FinanceTransactionRow {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            tx_type: TransactionType::Expense.as_str().to_string(),
            amount,
            currency: currency.clone(),
            category: category.map(|s| s.to_string()),
            subcategory: subcategory.map(|s| s.to_string()),
            counterparty: counterparty.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
            tx_date,
            transfer_id: Some(transfer_id.clone()),
            is_recurring: false,
            recurring_rule: None,
            created_at: now,
            updated_at: now,
        };

        let to_row = FinanceTransactionRow {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: dest_id.to_string(),
            tx_type: TransactionType::Income.as_str().to_string(),
            amount,
            currency,
            category: category.map(|s| s.to_string()),
            subcategory: subcategory.map(|s| s.to_string()),
            counterparty: counterparty.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
            tx_date,
            transfer_id: Some(transfer_id.clone()),
            is_recurring: false,
            recurring_rule: None,
            created_at: now,
            updated_at: now,
        };

        let from_inserted = self.storage.transactions.add(&from_row).await?;

        let to_inserted = self.storage.transactions.add(&to_row).await?;

        let from_updated = self
            .storage
            .accounts
            .adjust_balance(account_id, -amount)
            .await?;

        let to_updated = self
            .storage
            .accounts
            .adjust_balance(dest_id, amount)
            .await?;

        let result = json!({
            "transfer_id": transfer_id,
            "from_tx": {
                "id": from_inserted.id,
                "account_id": from_inserted.account_id,
                "type": "expense",
                "amount": from_inserted.amount,
                "currency": from_inserted.currency,
                "tx_date": from_inserted.tx_date.to_string(),
            },
            "to_tx": {
                "id": to_inserted.id,
                "account_id": to_inserted.account_id,
                "type": "income",
                "amount": to_inserted.amount,
                "currency": to_inserted.currency,
                "tx_date": to_inserted.tx_date.to_string(),
            },
            "from_balance": from_updated.balance,
            "to_balance": to_updated.balance,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }
}
