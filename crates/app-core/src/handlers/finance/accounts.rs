use desktop_shared::commands::{FinanceAccountCreateParams, FinanceAccountUpdateParams};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{FinanceAccountPatch, FinanceAccountRow};

use crate::errors::map_storage_err;
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    pub async fn finance_accounts(&self) -> Result<Vec<FinanceAccountRow>, ApiError> {
        self.repos
            .finance
            .accounts
            .list(false)
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_account_create(
        &self,
        params: FinanceAccountCreateParams,
    ) -> HandlerResult<FinanceAccountRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceAccountRow {
            id: id.clone(),
            name: params.name,
            account_type: params.account_type,
            currency,
            balance: params.balance.unwrap_or(0),
            institution: params.institution,
            notes: params.notes,
            is_archived: false,
            created_at: now,
            updated_at: now,
            base_balance: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        };

        self.repos
            .finance
            .accounts
            .add(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_account_update(
        &self,
        params: FinanceAccountUpdateParams,
    ) -> HandlerResult<FinanceAccountRow> {
        let patch = FinanceAccountPatch {
            id: params.id.clone(),
            name: params.name,
            balance: params.balance,
            institution: params.institution,
            notes: params.notes,
            is_archived: params.is_archived,
            base_balance: None,
            base_currency: None,
            exchange_rate: None,
        };
        let row = self
            .repos
            .finance
            .accounts
            .update(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    pub async fn finance_account_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos
            .finance
            .accounts
            .delete(&id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }
}
