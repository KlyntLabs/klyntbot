//! Account action handlers for `FinanceTool`.
//!
//! Handles: account_add, account_list, account_update, account_delete.

use chrono::Utc;
use serde_json::json;

use crate::currency::ensure_base_amount;
use crate::types::{AccountType, FinanceAccount};
use common::{Result, ToolError};
use storage::rows::finance::{FinanceAccountPatch, FinanceAccountRow, FinanceTransactionFilter};
use storage::StorageError;
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::FinanceTool;

impl FinanceTool {
    pub(crate) async fn handle_account(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "account_add" => self.account_add(p, ctx).await,
            "account_list" => self.account_list(p).await,
            "account_update" => self.account_update(p).await,
            "account_delete" => self.account_delete(p).await,
            _ => {
                Err(ToolError::InvalidParams(format!("Unknown account action: {}", action)).into())
            }
        }
    }

    async fn account_add(&self, p: &ParamExtractor<'_>, ctx: &RoutingContext) -> Result<String> {
        let name = p.required_str("name")?;
        if name.is_empty() {
            return Err(ToolError::InvalidParams("Account name is required".to_string()).into());
        }

        let type_str = p.required_str("type")?;
        let account_type = AccountType::from_str_loose(type_str).ok_or_else(|| {
            ToolError::InvalidParams(format!("Invalid account type: {}", type_str))
        })?;

        let currency = p
            .optional_str("currency")?
            .unwrap_or(&self.default_currency);
        if currency.is_empty() || currency.len() > 3 {
            return Err(ToolError::InvalidParams(
                "Currency must be a valid ISO 4217 code".to_string(),
            )
            .into());
        }

        let balance = p.i64_or("balance", 0)?;
        let institution = p.optional_str("institution")?;
        let notes = p.optional_str("notes")?;

        let now = Utc::now();
        let id = uuid::Uuid::new_v4().to_string();

        let conv = ensure_base_amount(
            balance,
            currency,
            &self.default_currency,
            &self.price_service,
        )
        .await?;

        let row = FinanceAccountRow {
            id,
            name: name.to_string(),
            account_type: account_type.as_str().to_string(),
            currency: currency.to_uppercase(),
            balance,
            institution: institution.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
            is_archived: false,
            created_at: now,
            updated_at: now,
            base_balance: conv.base_amount,
            base_currency: conv.base_currency,
            exchange_rate: conv.exchange_rate,
        };

        let inserted = self.storage.accounts.add(&row).await?;

        let account = FinanceAccount::from(inserted);

        if let Some(ref tx) = ctx.entity_tx {
            let _ = tx
                .send(common::EntityCard {
                    entity_type: "finance_account".to_string(),
                    entity_id: account.id.clone(),
                    title: account.name.clone(),
                    subtitle: Some(format!(
                        "{} · {}",
                        account.account_type.as_str(),
                        account.currency
                    )),
                    route: Some("/finance".to_string()),
                    icon_hint: "dollar".to_string(),
                    metadata: std::collections::HashMap::new(),
                })
                .await;
        }

        let result = json!({
            "id": account.id,
            "name": account.name,
            "account_type": account.account_type.as_str(),
            "currency": account.currency,
            "balance": account.balance,
            "institution": account.institution,
            "created_at": account.created_at,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn account_list(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let include_archived = p.optional_bool("is_archived")?.unwrap_or(false);
        let currency = p.optional_str("currency")?;

        let rows = if let Some(c) = currency {
            self.storage.accounts.list_by_currency(c).await?
        } else {
            self.storage.accounts.list(include_archived).await?
        };

        if rows.is_empty() {
            return Ok("No accounts found.".to_string());
        }

        let accounts: Vec<FinanceAccount> = rows.into_iter().map(FinanceAccount::from).collect();

        let total_by_currency = self.storage.accounts.total_balance_by_currency().await?;

        let total_map: serde_json::Map<String, serde_json::Value> = total_by_currency
            .into_iter()
            .map(|(cur, total)| (cur, json!(total)))
            .collect();

        let account_json: Vec<serde_json::Value> = accounts
            .iter()
            .map(|a| {
                json!({
                    "id": a.id,
                    "name": a.name,
                    "account_type": a.account_type.as_str(),
                    "currency": a.currency,
                    "balance": a.balance,
                    "institution": a.institution,
                    "is_archived": a.is_archived,
                })
            })
            .collect();

        let result = json!({
            "accounts": account_json,
            "total_by_currency": serde_json::Value::Object(total_map),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn account_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let name = p.optional_str("name")?;
        let balance = p.optional_i64("balance")?;
        let institution = p.optional_str("institution")?;
        let notes = p.optional_str("notes")?;
        let is_archived = p.optional_bool("is_archived")?;

        if name.is_none()
            && balance.is_none()
            && institution.is_none()
            && notes.is_none()
            && is_archived.is_none()
        {
            return Err(ToolError::InvalidParams("No fields to update".to_string()).into());
        }

        // If the balance is being updated, recompute base conversion.
        let (base_balance, base_currency, exchange_rate) = if let Some(new_balance) = balance {
            let account_row =
                self.storage.accounts.get(id).await?.ok_or_else(|| {
                    ToolError::ExecutionFailed(format!("Account {} not found", id))
                })?;
            let conv = ensure_base_amount(
                new_balance,
                &account_row.currency,
                &self.default_currency,
                &self.price_service,
            )
            .await?;
            (
                Some(conv.base_amount),
                Some(conv.base_currency),
                Some(conv.exchange_rate),
            )
        } else {
            (None, None, None)
        };

        let patch = FinanceAccountPatch {
            id: id.to_string(),
            name: name.map(|s| s.to_string()),
            balance,
            institution: institution.map(|s| Some(s.to_string())),
            notes: notes.map(|s| Some(s.to_string())),
            is_archived,
            base_balance,
            base_currency,
            exchange_rate,
        };

        let row = self
            .storage
            .accounts
            .update(&patch)
            .await
            .map_err(|e| match e {
                StorageError::NotFound(_) => {
                    ToolError::ExecutionFailed(format!("Account {} not found", id))
                }
                other => ToolError::ExecutionFailed(other.to_string()),
            })?;

        let account = FinanceAccount::from(row);
        let result = json!({
            "id": account.id,
            "name": account.name,
            "account_type": account.account_type.as_str(),
            "currency": account.currency,
            "balance": account.balance,
            "institution": account.institution,
            "is_archived": account.is_archived,
            "updated_at": account.updated_at,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn account_delete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let exists = self.storage.accounts.get(id).await?;

        if exists.is_none() {
            return Err(ToolError::ExecutionFailed(format!("Account {} not found", id)).into());
        }

        let tx_filter = FinanceTransactionFilter {
            account_id: Some(id.to_string()),
            limit: Some(i64::MAX),
            ..Default::default()
        };
        let tx_count = self.storage.transactions.list(&tx_filter).await?.len();

        self.storage.accounts.delete(id).await?;

        let msg = if tx_count == 0 {
            "Account deleted. No transactions removed.".to_string()
        } else {
            format!("Account deleted. {} transactions removed.", tx_count)
        };

        let result = json!({
            "deleted": true,
            "message": msg,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }
}
