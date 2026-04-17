use desktop_shared::commands::{FinanceBudgetCreateParams, FinanceBudgetUpdateParams};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{BudgetUsageRow, FinanceBudgetPatch, FinanceBudgetRow};

use crate::errors::{map_storage_err, parse_naive_date};
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    pub async fn finance_budget_usage(&self) -> Result<Vec<BudgetUsageRow>, ApiError> {
        self.repos
            .finance
            .budgets
            .all_budget_usage()
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_budget_create(
        &self,
        params: FinanceBudgetCreateParams,
    ) -> HandlerResult<FinanceBudgetRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now_chrono = chrono::Utc::now();
        let now: storage::SqlTs = common::time::bridge::chrono_to_jiff(now_chrono).into();
        let start_date_naive = params
            .start_date
            .and_then(|d| parse_naive_date(&d))
            .unwrap_or_else(|| now_chrono.date_naive());
        let start_date: storage::SqlDate =
            common::time::bridge::chrono_date_to_jiff(start_date_naive).into();
        let end_date: Option<storage::SqlDate> = params
            .end_date
            .and_then(|d| parse_naive_date(&d))
            .map(|d| common::time::bridge::chrono_date_to_jiff(d).into());
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceBudgetRow {
            id: id.clone(),
            name: params.name,
            amount: params.amount,
            currency,
            period: params.period,
            category: params.category,
            method: params.method.unwrap_or_else(|| "standard".to_string()),
            jar_type: None,
            start_date,
            end_date,
            is_active: true,
            alert_threshold: params.alert_threshold.unwrap_or(80),
            created_at: now,
            updated_at: now,
            base_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        };

        self.repos
            .finance
            .budgets
            .add(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_budget_update(
        &self,
        params: FinanceBudgetUpdateParams,
    ) -> HandlerResult<FinanceBudgetRow> {
        let patch = FinanceBudgetPatch {
            id: params.id.clone(),
            name: params.name,
            amount: params.amount,
            category: params.category,
            is_active: params.is_active,
            base_amount: None,
            base_currency: None,
            exchange_rate: None,
        };
        let row = self
            .repos
            .finance
            .budgets
            .update(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    pub async fn finance_budget_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos
            .finance
            .budgets
            .delete(&id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }
}
