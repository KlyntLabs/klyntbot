use desktop_shared::commands::{
    FinanceAllocationTargetUpsertParams, FinanceInvestmentCreateParams,
    FinanceInvestmentTxCreateParams, FinanceInvestmentUpdateParams, FinancePortfolioCreateParams,
    FinancePortfolioResponse,
};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{
    FinanceAllocationTargetRow, FinanceInvestmentFilter, FinanceInvestmentPatch,
    FinanceInvestmentRow, FinanceInvestmentTxRow, FinancePortfolioRow,
};

use crate::errors::{map_storage_err, parse_naive_date};
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn finance_portfolios(&self) -> Result<Vec<FinancePortfolioResponse>, ApiError> {
        let portfolios = feature_finance::api::list_portfolios(&self.repos.finance)
            .await
            .map_err(map_storage_err)?;

        let default_currency = self.default_currency().await;
        let summaries =
            feature_finance::api::portfolio_summaries(&self.repos.finance, &default_currency)
                .await
                .map_err(map_storage_err)?;

        Ok(portfolios
            .into_iter()
            .zip(summaries)
            .map(|(p, summary)| FinancePortfolioResponse {
                id: p.id,
                name: p.name,
                description: p.description,
                currency: p.currency,
                total_value: summary.total_current_value,
                total_cost_basis: summary.total_cost_basis,
                holding_count: summary.holding_count,
            })
            .collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_investments(&self) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        feature_finance::api::list_investments(&self.repos.finance, &Default::default())
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_portfolio_create(
        &self,
        params: FinancePortfolioCreateParams,
    ) -> HandlerResult<FinancePortfolioRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };
        let row = FinancePortfolioRow {
            id: id.clone(),
            name: params.name,
            description: params.description,
            currency,
            created_at: now,
            updated_at: now,
        };
        let row = feature_finance::api::create_portfolio(&self.repos.finance, &row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_investment_create(
        &self,
        params: FinanceInvestmentCreateParams,
    ) -> HandlerResult<FinanceInvestmentRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = jiff::Timestamp::now().into();
        let purchase_date = params
            .purchase_date
            .and_then(|d| parse_naive_date(&d))
            .map(|d| d.into());
        let currency = match params.currency {
            Some(c) => c,
            None => self.default_currency().await,
        };

        let row = FinanceInvestmentRow {
            id: id.clone(),
            portfolio_id: params.portfolio_id,
            asset_type: params.asset_type,
            symbol: params.symbol,
            name: params.name.unwrap_or_default(),
            quantity: params.quantity,
            cost_basis: params.cost_basis,
            currency,
            current_price: None,
            current_value: None,
            purchase_date,
            asset_class: None,
            notes: params.notes,
            created_at: now,
            updated_at: now,
            market_currency: None,
            base_cost_basis: 0,
            base_current_value: 0,
            base_currency: "USD".to_string(),
            purchase_rate: 1.0,
            market_rate: 1.0,
        };
        let row = feature_finance::api::create_investment(&self.repos.finance, &row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self))]
    pub async fn finance_investment_update(
        &self,
        params: FinanceInvestmentUpdateParams,
    ) -> HandlerResult<FinanceInvestmentRow> {
        let patch = FinanceInvestmentPatch {
            id: params.id.clone(),
            current_price: params.current_price,
            current_value: params.current_value,
            quantity: params.quantity,
            notes: params.notes,
            ..Default::default()
        };
        let row = feature_finance::api::update_investment(&self.repos.finance, &patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_investments_filtered(
        &self,
        portfolio_id: Option<String>,
    ) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        let filter = FinanceInvestmentFilter {
            portfolio_id,
            ..Default::default()
        };
        feature_finance::api::list_investments(&self.repos.finance, &filter)
            .await
            .map_err(map_storage_err)
    }

    // ── Allocation Targets ────────────────────────────────────────

    #[tracing::instrument(skip(self))]
    pub async fn finance_allocation_target_upsert(
        &self,
        params: FinanceAllocationTargetUpsertParams,
    ) -> HandlerResult<FinanceAllocationTargetRow> {
        let row = feature_finance::api::upsert_allocation_target(
            &self.repos.finance,
            &params.portfolio_id,
            &params.asset_class,
            &params.target_weight,
            &params.tolerance_band,
        )
        .await
        .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.portfolio_id)))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_allocation_targets(
        &self,
        portfolio_id: String,
    ) -> Result<Vec<FinanceAllocationTargetRow>, ApiError> {
        feature_finance::api::list_allocation_targets(&self.repos.finance, &portfolio_id)
            .await
            .map_err(map_storage_err)
    }

    // ── Investment Transactions ────────────────────────────────────

    #[tracing::instrument(skip(self))]
    pub async fn finance_investment_tx_create(
        &self,
        params: FinanceInvestmentTxCreateParams,
    ) -> HandlerResult<FinanceInvestmentTxRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();
        let tx_date: storage::SqlDate = parse_naive_date(&params.tx_date)
            .ok_or_else(|| ApiError::new("VALIDATION", "invalid txDate format"))
            .map(|d| d.into())?;
        let row = FinanceInvestmentTxRow {
            id: id.clone(),
            investment_id: params.investment_id,
            tx_type: params.tx_type,
            quantity: params.quantity,
            price_per_unit: params.price_per_unit,
            total_amount: params.total_amount,
            currency: params.currency.clone(),
            fees: params.fees.unwrap_or(0),
            tx_date,
            notes: params.notes,
            created_at: now,
            base_total_amount: params.total_amount,
            base_currency: params.currency,
            exchange_rate: 1.0,
        };
        let row = feature_finance::api::create_investment_tx(&self.repos.finance, &row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn finance_investment_txs(
        &self,
        investment_id: String,
    ) -> Result<Vec<FinanceInvestmentTxRow>, ApiError> {
        feature_finance::api::list_investment_txs(&self.repos.finance, &investment_id)
            .await
            .map_err(map_storage_err)
    }
}
