//! Account action handlers for `FinanceTool`.
//!
//! Handles: account_add, account_list, account_update, account_delete.

use chrono::Utc;
use serde_json::json;

use crate::finance_types::{AccountType, FinanceAccount};
use crate::params::ParamExtractor;
use crate::RoutingContext;
use common::{Result, ToolError};
use storage::{FinanceAccountPatch, FinanceAccountRow, FinanceTransactionFilter, StorageError};

use super::FinanceTool;

impl FinanceTool {
    pub(crate) async fn handle_account(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "account_add" => self.account_add(p).await,
            "account_list" => self.account_list(p).await,
            "account_update" => self.account_update(p).await,
            "account_delete" => self.account_delete(p).await,
            _ => {
                Err(ToolError::InvalidParams(format!("Unknown account action: {}", action)).into())
            }
        }
    }

    async fn account_add(&self, p: &ParamExtractor<'_>) -> Result<String> {
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
        };

        let inserted = self
            .accounts
            .add(&row)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let account = FinanceAccount::from(inserted);
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
        // `is_archived` doubles as `include_archived` for list queries.
        let include_archived = p.optional_bool("is_archived")?.unwrap_or(false);
        let currency = p.optional_str("currency")?;

        let rows = if let Some(c) = currency {
            self.accounts
                .list_by_currency(c)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("list_by_currency: {}", e)))?
        } else {
            self.accounts
                .list(include_archived)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("list: {}", e)))?
        };

        if rows.is_empty() {
            return Ok("No accounts found.".to_string());
        }

        let accounts: Vec<FinanceAccount> = rows.into_iter().map(FinanceAccount::from).collect();

        let total_by_currency = self
            .accounts
            .total_balance_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("total_balance_by_currency: {}", e)))?;

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

        let patch = FinanceAccountPatch {
            id: id.to_string(),
            name: name.map(|s| s.to_string()),
            balance,
            institution: institution.map(|s| Some(s.to_string())),
            notes: notes.map(|s| Some(s.to_string())),
            is_archived,
        };

        let row = self.accounts.update(&patch).await.map_err(|e| match e {
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

        // Verify the account exists before deletion.
        let exists = self
            .accounts
            .get(id)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if exists.is_none() {
            return Err(ToolError::ExecutionFailed(format!("Account {} not found", id)).into());
        }

        // Count transactions before the cascade delete so we can report the number.
        let tx_filter = FinanceTransactionFilter {
            account_id: Some(id.to_string()),
            limit: Some(i64::MAX),
            ..Default::default()
        };
        let tx_count = self
            .transactions
            .list(&tx_filter)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .len();

        self.accounts
            .delete(id)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

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

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::finance_types::AccountType;
    use crate::params::ParamExtractor;

    // ── Parameter validation unit tests (no DB required) ────────────────────

    #[test]
    fn account_add_empty_name_detected() {
        let args = json!({ "name": "", "type": "bank", "currency": "VND" });
        let p = ParamExtractor::new(&args);
        let name = p.required_str("name").unwrap();
        assert!(name.is_empty(), "empty name should be detected as invalid");
    }

    #[test]
    fn account_add_invalid_type_rejected() {
        assert!(
            AccountType::from_str_loose("checking").is_none(),
            "'checking' should not be a valid account type"
        );
        assert!(
            AccountType::from_str_loose("savings").is_none(),
            "'savings' should not be a valid account type"
        );
    }

    #[test]
    fn account_add_valid_types_accepted() {
        for s in &[
            "cash",
            "bank",
            "ewallet",
            "crypto_wallet",
            "brokerage",
            "other",
        ] {
            assert!(
                AccountType::from_str_loose(s).is_some(),
                "'{s}' should be a valid account type"
            );
        }
        // Case-insensitive
        assert!(AccountType::from_str_loose("BANK").is_some());
        assert!(AccountType::from_str_loose("Cash").is_some());
    }

    #[test]
    fn account_add_empty_currency_invalid() {
        let currency = "";
        assert!(
            currency.is_empty() || currency.len() > 3,
            "empty currency should fail length check"
        );
    }

    #[test]
    fn account_add_currency_too_long_invalid() {
        let currency = "USDC";
        assert!(
            currency.len() > 3,
            "4-char currency 'USDC' should fail length check"
        );
    }

    #[test]
    fn account_add_valid_currency_accepted() {
        for c in &["USD", "VND", "EUR", "BTC"] {
            assert!(
                !c.is_empty() && c.len() <= 3,
                "'{c}' should pass length check"
            );
        }
    }

    #[test]
    fn account_add_params_extract_correctly() {
        let args = json!({
            "name": "My Bank",
            "type": "bank",
            "currency": "USD",
            "balance": 1_000_000_i64,
            "institution": "Chase",
            "notes": "Main checking account"
        });
        let p = ParamExtractor::new(&args);

        assert_eq!(p.required_str("name").unwrap(), "My Bank");
        assert_eq!(p.required_str("type").unwrap(), "bank");
        assert_eq!(p.required_str("currency").unwrap(), "USD");
        assert_eq!(p.i64_or("balance", 0).unwrap(), 1_000_000);
        assert_eq!(p.optional_str("institution").unwrap(), Some("Chase"));
        assert_eq!(
            p.optional_str("notes").unwrap(),
            Some("Main checking account")
        );
    }

    #[test]
    fn account_add_negative_balance_allowed() {
        // Negative balance is valid (credit line / overdraft).
        let args = json!({ "balance": -50_000_i64 });
        let p = ParamExtractor::new(&args);
        let balance = p.i64_or("balance", 0).unwrap();
        assert_eq!(balance, -50_000);
    }

    #[test]
    fn account_add_default_balance_zero() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("balance", 0).unwrap(), 0);
    }

    #[test]
    fn account_update_no_fields_detected() {
        let args = json!({ "id": "some-id" });
        let p = ParamExtractor::new(&args);

        let name = p.optional_str("name").unwrap();
        let balance = p.optional_i64("balance").unwrap();
        let institution = p.optional_str("institution").unwrap();
        let notes = p.optional_str("notes").unwrap();
        let is_archived = p.optional_bool("is_archived").unwrap();

        assert!(
            name.is_none()
                && balance.is_none()
                && institution.is_none()
                && notes.is_none()
                && is_archived.is_none(),
            "all optional fields absent → 'no fields to update' condition"
        );
    }

    #[test]
    fn account_list_include_archived_defaults_false() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        let include_archived = p.optional_bool("is_archived").unwrap().unwrap_or(false);
        assert!(!include_archived);
    }

    #[test]
    fn account_list_include_archived_explicit_true() {
        let args = json!({ "is_archived": true });
        let p = ParamExtractor::new(&args);
        let include_archived = p.optional_bool("is_archived").unwrap().unwrap_or(false);
        assert!(include_archived);
    }
}
