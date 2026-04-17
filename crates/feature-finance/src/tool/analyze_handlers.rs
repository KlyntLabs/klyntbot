//! Spending analytics action handlers for `FinanceTool`.
//!
//! Handles: analyze_spending_anomalies, analyze_spending_trends,
//! analyze_recurring_charges, analyze_category_correlation.

use chrono::{Duration, Local};
use common::{Decimal, Result, ToolError};
use serde_json::json;
use storage::rows::finance::FinanceTransactionFilter;
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use analytics::input_types::{SpendingRecord, SpendingType};
use analytics::spending::{
    AnomalyConfig, CorrelationConfig, RecurringConfig, SpendingAnalyzer, TrendConfig,
};

use super::FinanceTool;

impl FinanceTool {
    pub(crate) async fn handle_analyze(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "analyze_spending_anomalies" => self.analyze_spending_anomalies(p).await,
            "analyze_spending_trends" => self.analyze_spending_trends(p).await,
            "analyze_recurring_charges" => self.analyze_recurring_charges(p).await,
            "analyze_category_correlation" => self.analyze_category_correlation(p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown analyze action: {action}")).into()),
        }
    }

    /// Fetch transactions for the given lookback period and convert to SpendingRecords.
    async fn fetch_spending_records(&self, lookback_months: i64) -> Result<Vec<SpendingRecord>> {
        let today = Local::now().date_naive();
        let date_from = today - Duration::days(lookback_months * 30);

        let filter = FinanceTransactionFilter {
            date_from: Some(date_from),
            date_to: Some(today),
            limit: Some(10_000),
            ..Default::default()
        };

        let rows = self.storage.transactions.list(&filter).await?;

        let records: Vec<SpendingRecord> = rows
            .into_iter()
            .filter_map(|row| {
                let tx_type = match row.tx_type.as_str() {
                    "expense" => SpendingType::Expense,
                    "income" => SpendingType::Income,
                    _ => return None, // skip transfers
                };
                Some(SpendingRecord {
                    date: common::time::bridge::chrono_date_to_jiff(row.tx_date),
                    amount: Decimal::new(row.amount, 0),
                    tx_type,
                    category: row.category,
                    counterparty: row.counterparty,
                })
            })
            .collect();

        Ok(records)
    }

    async fn analyze_spending_anomalies(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let lookback_months = p.i64_or("lookback_months", 6)?;
        let z_threshold = p.optional_f64("z_threshold")?;

        let records = self.fetch_spending_records(lookback_months).await?;

        if records.is_empty() {
            return Ok("No transactions found for the specified period.".to_string());
        }

        let mut config = AnomalyConfig::default();
        if let Some(z) = z_threshold {
            config.z_threshold = Decimal::from_f64_retain(z).unwrap_or(config.z_threshold);
        }

        let anomalies = SpendingAnalyzer::detect_anomalies(&records, &config);

        let result: Vec<serde_json::Value> = anomalies
            .iter()
            .map(|a| {
                json!({
                    "date": a.date.to_string(),
                    "category": a.category,
                    "amount": a.amount.to_string(),
                    "z_score": a.z_score.to_string(),
                    "severity": a.severity,
                    "explanation": a.explanation,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&json!({
            "anomalies": result,
            "count": anomalies.len(),
            "lookback_months": lookback_months,
        }))
        .unwrap())
    }

    async fn analyze_spending_trends(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let lookback_months = p.i64_or("lookback_months", 6)?;
        let window = p.optional_i64("window_months")?;

        let records = self.fetch_spending_records(lookback_months).await?;

        if records.is_empty() {
            return Ok("No transactions found for the specified period.".to_string());
        }

        let mut config = TrendConfig::default();
        if let Some(w) = window {
            config.window_months = w.clamp(1, 24) as u32;
        }

        let report = SpendingAnalyzer::trends(&records, &config);

        Ok(serde_json::to_string_pretty(&json!({
            "overall_direction": report.overall_direction,
            "monthly_totals": report.monthly_totals.iter()
                .map(|(label, amount)| json!({"month": label, "amount": amount.to_string()}))
                .collect::<Vec<_>>(),
            "moving_average": report.moving_average.iter()
                .map(|(label, amount)| json!({"month": label, "amount": amount.to_string()}))
                .collect::<Vec<_>>(),
            "period_over_period": report.period_over_period.iter()
                .map(|(label, pct)| json!({"month": label, "change_pct": pct.to_string()}))
                .collect::<Vec<_>>(),
            "category_trends": report.category_trends.iter()
                .map(|ct| json!({
                    "category": ct.category,
                    "direction": ct.direction,
                    "average_monthly": ct.average_monthly.to_string(),
                    "latest_monthly": ct.latest_monthly.to_string(),
                    "change_pct": ct.change_pct.to_string(),
                }))
                .collect::<Vec<_>>(),
            "lookback_months": lookback_months,
        }))
        .unwrap())
    }

    async fn analyze_recurring_charges(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let lookback_months = p.i64_or("lookback_months", 12)?;
        let min_occurrences = p.optional_i64("min_occurrences")?;

        let records = self.fetch_spending_records(lookback_months).await?;

        if records.is_empty() {
            return Ok("No transactions found for the specified period.".to_string());
        }

        let mut config = RecurringConfig::default();
        if let Some(min) = min_occurrences {
            config.min_occurrences = min.clamp(2, 100) as usize;
        }
        config.max_lookback_days = lookback_months * 30;

        let today = common::time::bridge::chrono_date_to_jiff(Local::now().date_naive());
        let charges = SpendingAnalyzer::detect_recurring(&records, &config, today);

        let result: Vec<serde_json::Value> = charges
            .iter()
            .map(|c| {
                json!({
                    "counterparty": c.counterparty,
                    "frequency": c.frequency,
                    "average_amount": c.average_amount.to_string(),
                    "confidence": c.confidence.to_string(),
                    "annual_cost": c.annual_cost.to_string(),
                    "last_date": c.last_date.to_string(),
                    "is_overdue": c.is_overdue,
                    "occurrences": c.occurrences,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&json!({
            "recurring_charges": result,
            "count": charges.len(),
            "lookback_months": lookback_months,
        }))
        .unwrap())
    }

    async fn analyze_category_correlation(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let lookback_months = p.i64_or("lookback_months", 12)?;
        let min_months = p.optional_i64("min_months")?;

        let records = self.fetch_spending_records(lookback_months).await?;

        if records.is_empty() {
            return Ok("No transactions found for the specified period.".to_string());
        }

        let mut config = CorrelationConfig::default();
        if let Some(min) = min_months {
            config.min_months = min.clamp(2, 60) as usize;
        }

        let matrix = SpendingAnalyzer::category_correlation(&records, &config);

        // Format coefficients as strings for JSON
        let coefficients: Vec<Vec<String>> = matrix
            .coefficients
            .iter()
            .map(|row| row.iter().map(|c| c.to_string()).collect())
            .collect();

        Ok(serde_json::to_string_pretty(&json!({
            "categories": matrix.labels,
            "correlation_matrix": coefficients,
            "lookback_months": lookback_months,
        }))
        .unwrap())
    }
}
