use desktop_shared::commands::{FinanceTransactionCreateParams, FinanceTransactionFilterParams};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{FinanceTransactionFilter, FinanceTransactionRow};

use crate::errors::{map_storage_err, parse_naive_date};
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    pub async fn finance_transactions(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<FinanceTransactionRow>, ApiError> {
        let filter = FinanceTransactionFilter {
            limit: Some(limit.unwrap_or(100).min(500)),
            ..Default::default()
        };
        self.repos
            .finance
            .transactions
            .list(&filter)
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_transaction_create(
        &self,
        params: FinanceTransactionCreateParams,
    ) -> HandlerResult<FinanceTransactionRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now_chrono = chrono::Utc::now();
        let now: storage::SqlTs = common::time::bridge::chrono_to_jiff(now_chrono).into();
        let tx_date_naive = params
            .tx_date
            .and_then(|d| parse_naive_date(&d))
            .unwrap_or_else(|| now_chrono.date_naive());
        let tx_date: storage::SqlDate =
            common::time::bridge::chrono_date_to_jiff(tx_date_naive).into();

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
            account_id: params.account_id.clone(),
            tx_type: params.tx_type.clone(),
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

        self.repos
            .finance
            .transactions
            .add(&row)
            .await
            .map_err(map_storage_err)?;

        // Adjust account balance
        let delta = match params.tx_type.as_str() {
            "income" => params.amount,
            "expense" => -params.amount,
            _ => 0,
        };
        if delta != 0 {
            self.repos
                .finance
                .accounts
                .adjust_balance(&params.account_id, delta)
                .await
                .map_err(map_storage_err)?;
        }

        // Emit domain event for timeline tracking
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::TransactionRecorded {
                category: row.category.clone().unwrap_or_default(),
                amount: row.amount as f64 / 100.0,
                is_over_budget: false,
            });
        }

        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_transaction_delete(&self, id: String) -> HandlerResult<bool> {
        let tx = self
            .repos
            .finance
            .transactions
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        if let Some(tx) = tx {
            // Reverse the balance adjustment
            let delta = match tx.tx_type.as_str() {
                "income" => -tx.amount,
                "expense" => tx.amount,
                _ => 0,
            };
            if delta != 0 {
                self.repos
                    .finance
                    .accounts
                    .adjust_balance(&tx.account_id, delta)
                    .await
                    .map_err(map_storage_err)?;
            }
        }

        Ok((true, Self::finance_updates(id)))
    }

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
                .map(|d| common::time::bridge::chrono_date_to_jiff(d).into()),
            date_to: params
                .date_to
                .and_then(|d| parse_naive_date(&d))
                .map(|d| common::time::bridge::chrono_date_to_jiff(d).into()),
            query: params.query,
            limit: params.limit,
            ..Default::default()
        };
        self.repos
            .finance
            .transactions
            .list(&filter)
            .await
            .map_err(map_storage_err)
    }
}
