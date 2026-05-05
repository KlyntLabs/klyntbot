use desktop_shared::commands::{FinanceAccountCreateParams, FinanceAccountUpdateParams};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{FinanceAccountPatch, FinanceAccountRow};

use crate::errors::map_storage_err;
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn finance_accounts(&self) -> Result<Vec<FinanceAccountRow>, ApiError> {
        feature_finance::api::list_accounts(&self.repos.finance)
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_account_create(
        &self,
        params: FinanceAccountCreateParams,
    ) -> HandlerResult<FinanceAccountRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();
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

        let row = feature_finance::api::create_account(&self.repos.finance, &row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self))]
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
        let row = feature_finance::api::update_account(&self.repos.finance, &patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_account_delete(&self, id: String) -> HandlerResult<bool> {
        feature_finance::api::delete_account(&self.repos.finance, &id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }
}
