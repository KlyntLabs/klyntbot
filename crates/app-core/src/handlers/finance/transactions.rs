use desktop_shared::commands::{FinanceTransactionCreateParams, FinanceTransactionFilterParams};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{FinanceTransactionFilter, FinanceTransactionRow};

use crate::errors::{map_storage_err, parse_naive_date};
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn finance_transactions(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<FinanceTransactionRow>, ApiError> {
        let filter = FinanceTransactionFilter {
            limit: Some(limit.unwrap_or(100).min(500)),
            ..Default::default()
        };
        feature_finance::api::list_transactions(&self.repos.finance, &filter)
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_transaction_create(
        &self,
        params: FinanceTransactionCreateParams,
    ) -> HandlerResult<FinanceTransactionRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();
        let tx_date: storage::SqlDate = params
            .tx_date
            .and_then(|d| parse_naive_date(&d))
            .unwrap_or_else(|| jiff::Zoned::now().date())
            .into();

        let account = self
            .repos
            .finance
            .accounts
            .get_or_err(&params.account_id)
            .await
            .map_err(map_storage_err)?;
        let currency = params.currency.unwrap_or(account.currency.clone());

        let row = FinanceTransactionRow {
            id: id.clone(),
            account_id: params.account_id,
            tx_type: params.tx_type,
            amount: params.amount,
            currency,
            category: params.category,
            subcategory: params.subcategory,
            counterparty: params.counterparty,
            notes: params.notes,
            tx_date,
            transfer_id: None,
            is_recurring: false,
            recurring_rule: None,
            created_at: now,
            updated_at: now,
            base_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        };

        let row = feature_finance::api::create_transaction(
            &self.repos.finance,
            &row,
            self.domain_event_bus.as_ref(),
        )
        .await
        .map_err(map_storage_err)?;

        Ok((row, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_transaction_delete(&self, id: String) -> HandlerResult<bool> {
        feature_finance::api::delete_transaction(&self.repos.finance, &id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_transactions_filtered(
        &self,
        params: FinanceTransactionFilterParams,
    ) -> Result<Vec<FinanceTransactionRow>, ApiError> {
        let filter = FinanceTransactionFilter {
            account_id: params.account_id,
            tx_type: params.tx_type,
            category: params.category,
            date_from: params
                .date_from
                .and_then(|d| parse_naive_date(&d))
                .map(|d| d.into()),
            date_to: params
                .date_to
                .and_then(|d| parse_naive_date(&d))
                .map(|d| d.into()),
            query: params.query,
            limit: params.limit,
            ..Default::default()
        };
        feature_finance::api::list_transactions(&self.repos.finance, &filter)
            .await
            .map_err(map_storage_err)
    }
}
