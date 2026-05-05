use std::collections::HashMap;

use desktop_shared::commands::{
    CurrencyNetWorth, DailySpending, FinanceCategoryBreakdown, FinanceCategoryReportResponse,
    FinanceDailySpendingResponse, FinanceGoalCreateParams, FinanceGoalUpdateParams,
    FinanceLiabilityCreateParams, FinanceLiabilityUpdateParams, FinanceMonthlySummaryResponse,
    FinanceNetWorthResponse, FinancePeriodSummaryResponse, FinanceTrendPoint,
};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{
    FinanceGoalPatch, FinanceGoalRow, FinanceLiabilityPatch, FinanceLiabilityRow,
};

use crate::errors::{map_storage_err, parse_naive_date};
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn finance_goals(&self) -> Result<Vec<FinanceGoalRow>, ApiError> {
        feature_finance::api::list_goals(&self.repos.finance)
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_liabilities(&self) -> Result<Vec<FinanceLiabilityRow>, ApiError> {
        feature_finance::api::list_liabilities(&self.repos.finance)
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_net_worth(&self) -> Result<FinanceNetWorthResponse, ApiError> {
        let totals = feature_finance::api::net_worth(&self.repos.finance)
            .await
            .map_err(map_storage_err)?;

        let totals_by_currency: Vec<CurrencyNetWorth> = totals
            .into_iter()
            .map(|(currency, accounts, investments, liabilities)| CurrencyNetWorth {
                currency,
                accounts,
                investments,
                liabilities,
                net: accounts + investments - liabilities,
            })
            .collect();

        Ok(FinanceNetWorthResponse { totals_by_currency })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_exchange_rates(&self) -> Result<HashMap<String, f64>, ApiError> {
        let config = self.config.read().await;
        Ok(config.finance.exchange_rates.clone().unwrap_or_default())
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_goal_create(
        &self,
        params: FinanceGoalCreateParams,
    ) -> HandlerResult<FinanceGoalRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();
        let deadline: Option<storage::SqlDate> = params
            .deadline
            .and_then(|d| parse_naive_date(&d))
            .map(|d| d.into());
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceGoalRow {
            id: id.clone(),
            name: params.name,
            goal_type: params.goal_type,
            target_amount: params.target_amount,
            current_amount: params.current_amount.unwrap_or(0),
            currency,
            status: "active".to_string(),
            deadline,
            monthly_contribution: params.monthly_contribution,
            expected_return_rate: None,
            inflation_rate: None,
            notes: params.notes,
            created_at: now,
            updated_at: now,
            base_target_amount: 0,
            base_current_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        };

        let row = feature_finance::api::create_goal(&self.repos.finance, &row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_goal_update(
        &self,
        params: FinanceGoalUpdateParams,
    ) -> HandlerResult<FinanceGoalRow> {
        let deadline: Option<Option<storage::SqlDate>> = params
            .deadline
            .map(|opt| opt.and_then(|d| parse_naive_date(&d)).map(|d| d.into()));
        let patch = FinanceGoalPatch {
            id: params.id.clone(),
            current_amount: params.current_amount,
            target_amount: params.target_amount,
            monthly_contribution: params.monthly_contribution,
            deadline,
            status: params.status,
            ..Default::default()
        };

        let row = feature_finance::api::update_goal(
            &self.repos.finance,
            &patch,
            self.domain_event_bus.as_ref(),
        )
        .await
        .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_goal_delete(&self, id: String) -> HandlerResult<bool> {
        feature_finance::api::delete_goal(&self.repos.finance, &id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_liability_create(
        &self,
        params: FinanceLiabilityCreateParams,
    ) -> HandlerResult<FinanceLiabilityRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();
        let due_date: Option<storage::SqlDate> = params
            .due_date
            .and_then(|d| parse_naive_date(&d))
            .map(|d| d.into());
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceLiabilityRow {
            id: id.clone(),
            name: params.name,
            liability_type: params.liability_type,
            principal: params.principal,
            remaining: params.remaining.unwrap_or(params.principal),
            currency,
            interest_rate: params.interest_rate,
            monthly_payment: params.monthly_payment,
            due_date,
            notes: params.notes,
            created_at: now,
            updated_at: now,
            base_principal: 0,
            base_remaining: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        };

        let row = feature_finance::api::create_liability(&self.repos.finance, &row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_liability_update(
        &self,
        params: FinanceLiabilityUpdateParams,
    ) -> HandlerResult<FinanceLiabilityRow> {
        let patch = FinanceLiabilityPatch {
            id: params.id.clone(),
            remaining: params.remaining,
            monthly_payment: params.monthly_payment,
            interest_rate: params.interest_rate,
            notes: params.notes,
            base_principal: None,
            base_remaining: None,
            base_currency: None,
            exchange_rate: None,
        };
        let row = feature_finance::api::update_liability(&self.repos.finance, &patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_liability_delete(&self, id: String) -> HandlerResult<bool> {
        feature_finance::api::delete_liability(&self.repos.finance, &id)
            .await
            .map_err(map_storage_err)?;
        Ok((true, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_report_spending(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<FinanceCategoryReportResponse, ApiError> {
        self.finance_report_by_type(date_from, date_to, "expense")
            .await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_report_income(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<FinanceCategoryReportResponse, ApiError> {
        self.finance_report_by_type(date_from, date_to, "income")
            .await
    }

    async fn finance_report_by_type(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
        tx_type: &str,
    ) -> Result<FinanceCategoryReportResponse, ApiError> {
        let now = jiff::Zoned::now().date();
        let from = date_from
            .and_then(|d| parse_naive_date(&d))
            .unwrap_or_else(|| now.with().day(1).build().unwrap_or(now));
        let to = date_to.and_then(|d| parse_naive_date(&d)).unwrap_or(now);

        let rows = feature_finance::api::category_report(
            &self.repos.finance,
            from,
            to,
            tx_type,
            &self.default_currency().await,
        )
        .await
        .map_err(map_storage_err)?;

        let total: i64 = rows.iter().map(|(_, amt)| amt).sum();
        let breakdown = rows
            .into_iter()
            .map(|(category, amount)| FinanceCategoryBreakdown {
                category,
                amount,
                pct: if total > 0 {
                    (amount as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();

        Ok(FinanceCategoryReportResponse { total, breakdown })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_report_trends(
        &self,
        metric: String,
        periods: Option<i64>,
    ) -> Result<Vec<FinanceTrendPoint>, ApiError> {
        let n = periods.unwrap_or(6).min(24);
        let tx_type = match metric.as_str() {
            "income" => "income",
            _ => "expense",
        };
        let rows = feature_finance::api::trend_report(
            &self.repos.finance,
            tx_type,
            n as i32,
            &self.default_currency().await,
        )
        .await
        .map_err(map_storage_err)?;

        let points: Vec<FinanceTrendPoint> = rows
            .iter()
            .enumerate()
            .map(|(i, (period, value))| {
                let change_pct = if i > 0 {
                    let prev = rows[i - 1].1;
                    if prev > 0 {
                        Some(((value - prev) as f64 / prev as f64) * 100.0)
                    } else {
                        None
                    }
                } else {
                    None
                };
                FinanceTrendPoint {
                    period: period.clone(),
                    value: *value,
                    change_pct,
                }
            })
            .collect();

        Ok(points)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_daily_spending(
        &self,
        date_from: String,
        date_to: String,
    ) -> Result<FinanceDailySpendingResponse, ApiError> {
        let from = parse_naive_date(&date_from).ok_or_else(|| {
            ApiError::new("INVALID_PARAMS", format!("invalid date_from: {date_from}"))
        })?;
        let to = parse_naive_date(&date_to).ok_or_else(|| {
            ApiError::new("INVALID_PARAMS", format!("invalid date_to: {date_to}"))
        })?;

        let rows = feature_finance::api::daily_spending(
            &self.repos.finance,
            from,
            to,
            &self.default_currency().await,
        )
        .await
        .map_err(map_storage_err)?;

        let days = rows
            .into_iter()
            .map(|(date, total_spending, tx_count)| DailySpending {
                date,
                total_spending,
                tx_count,
            })
            .collect();

        Ok(FinanceDailySpendingResponse { days })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_period_summary(
        &self,
        date_from: String,
        date_to: String,
    ) -> Result<FinancePeriodSummaryResponse, ApiError> {
        let from = parse_naive_date(&date_from).ok_or_else(|| {
            ApiError::new("INVALID_PARAMS", format!("invalid date_from: {date_from}"))
        })?;
        let to = parse_naive_date(&date_to).ok_or_else(|| {
            ApiError::new("INVALID_PARAMS", format!("invalid date_to: {date_to}"))
        })?;

        let (income, spending) = feature_finance::api::period_summary(
            &self.repos.finance,
            from,
            to,
            &self.default_currency().await,
        )
        .await
        .map_err(map_storage_err)?;

        Ok(FinancePeriodSummaryResponse { income, spending })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_monthly_summary(&self) -> Result<FinanceMonthlySummaryResponse, ApiError> {
        let ((_, current_income, current_spending), (_, previous_income, previous_spending)) =
            feature_finance::api::monthly_summary(&self.repos.finance, &self.default_currency().await)
                .await
                .map_err(map_storage_err)?;

        Ok(FinanceMonthlySummaryResponse {
            current_income,
            current_spending,
            previous_income,
            previous_spending,
        })
    }
}
