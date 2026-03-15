use desktop_shared::commands::{
    FinanceInvestmentCreateParams, FinanceInvestmentUpdateParams, FinancePortfolioCreateParams,
    FinancePortfolioResponse,
};
use desktop_shared::errors::ApiError;
use futures_util::future::try_join_all;
use storage::rows::finance::{
    FinanceInvestmentFilter, FinanceInvestmentPatch, FinanceInvestmentRow, FinancePortfolioRow,
};

use crate::errors::{map_storage_err, parse_naive_date};
use crate::state::{AppCore, HandlerResult};

impl AppCore {
    pub async fn finance_portfolios(&self) -> Result<Vec<FinancePortfolioResponse>, ApiError> {
        let portfolios = self
            .repos
            .finance
            .investments
            .list_portfolios()
            .await
            .map_err(map_storage_err)?;

        let default_currency = self.default_currency().await;
        let summaries = try_join_all(portfolios.iter().map(|p| {
            self.repos
                .finance
                .investments
                .portfolio_summary(&p.id, &default_currency)
        }))
        .await
        .map_err(map_storage_err)?;

        Ok(portfolios
            .iter()
            .zip(summaries)
            .map(|(p, summary)| FinancePortfolioResponse {
                id: p.id.clone(),
                name: p.name.clone(),
                description: p.description.clone(),
                currency: p.currency.clone(),
                total_value: summary.total_current_value,
                total_cost_basis: summary.total_cost_basis,
                holding_count: summary.holding_count,
            })
            .collect())
    }

    pub async fn finance_investments(&self) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        self.repos
            .finance
            .investments
            .list_investments(&Default::default())
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_portfolio_create(
        &self,
        params: FinancePortfolioCreateParams,
    ) -> HandlerResult<FinancePortfolioRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
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
        self.repos
            .finance
            .investments
            .add_portfolio(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

    pub async fn finance_investment_create(
        &self,
        params: FinanceInvestmentCreateParams,
    ) -> HandlerResult<FinanceInvestmentRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let purchase_date = params.purchase_date.and_then(|d| parse_naive_date(&d));
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
        self.repos
            .finance
            .investments
            .add_investment(&row)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(id)))
    }

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
        let row = self
            .repos
            .finance
            .investments
            .update_investment(&patch)
            .await
            .map_err(map_storage_err)?;
        Ok((row, Self::finance_updates(params.id)))
    }

    pub async fn finance_investments_filtered(
        &self,
        portfolio_id: Option<String>,
    ) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        let filter = FinanceInvestmentFilter {
            portfolio_id,
            ..Default::default()
        };
        self.repos
            .finance
            .investments
            .list_investments(&filter)
            .await
            .map_err(map_storage_err)
    }
}
