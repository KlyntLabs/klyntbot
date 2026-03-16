//! Report action handlers for `FinanceTool`.
//!
//! Handles: report_spending, report_income, report_trends,
//! report_net_worth_history, daily_review.

use chrono::{Datelike, Duration, Local, NaiveDate};
use serde_json::json;

use common::{Result, ToolError};
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::{parse_date, FinanceTool};

// ── Private helpers ──────────────────────────────────────────────────────────

/// Derive (date_from, date_to, period_label) from a period keyword and today's date.
fn derive_date_range(period: &str, today: NaiveDate) -> Result<(NaiveDate, NaiveDate, String)> {
    match period {
        "month" | "monthly" => {
            let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            let last = if today.month() == 12 {
                NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap() - Duration::days(1)
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap()
                    - Duration::days(1)
            };
            let label = today.format("%B %Y").to_string();
            Ok((first, last, label))
        }
        "week" | "weekly" => {
            let days_from_monday = today.weekday().num_days_from_monday() as i64;
            let first = today - Duration::days(days_from_monday);
            let last = first + Duration::days(6);
            let label = format!("Week of {}", first.format("%b %d, %Y"));
            Ok((first, last, label))
        }
        "quarter" | "quarterly" => {
            let quarter = (today.month() - 1) / 3;
            let start_month = quarter * 3 + 1;
            let end_month = start_month + 2;
            let first = NaiveDate::from_ymd_opt(today.year(), start_month, 1).unwrap();
            let last = if end_month == 12 {
                NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap() - Duration::days(1)
            } else {
                NaiveDate::from_ymd_opt(today.year(), end_month + 1, 1).unwrap() - Duration::days(1)
            };
            let label = format!("Q{} {}", quarter + 1, today.year());
            Ok((first, last, label))
        }
        "year" | "yearly" | "this_year" => {
            let first = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
            let last = NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap();
            let label = today.year().to_string();
            Ok((first, last, label))
        }
        "last_30_days" => {
            let first = today - Duration::days(29);
            let label = format!("Last 30 days ({first} to {today})");
            Ok((first, today, label))
        }
        "custom" => Err(ToolError::InvalidParams(
            "Period 'custom' requires explicit date_from and date_to parameters".to_string(),
        )
        .into()),
        _ => Err(ToolError::InvalidParams(format!(
            "Unknown period '{period}'. Use: month, week, quarter, year, last_30_days, this_year"
        ))
        .into()),
    }
}

/// Percentage change from `prev` to `curr`, or `None` when `prev` is zero.
fn change_pct(curr: i64, prev: i64) -> Option<i64> {
    if prev == 0 {
        None
    } else {
        Some((curr - prev) * 100 / prev)
    }
}

// ── FinanceTool impl ─────────────────────────────────────────────────────────

impl FinanceTool {
    pub(crate) async fn handle_report(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "report_spending" => self.report_spending(p).await,
            "report_income" => self.report_income(p).await,
            "report_trends" => self.report_trends(p).await,
            "report_net_worth_history" => self.report_net_worth_history(p).await,
            "daily_review" => self.daily_review_action().await,
            _ => Err(ToolError::InvalidParams(format!("Unknown report action: {action}")).into()),
        }
    }

    // ── report_spending ──────────────────────────────────────────────────────

    async fn report_spending(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let (date_from, date_to, period_label) = self.resolve_period_dates(p)?;
        let category = p.optional_str("category")?;

        let mut rows = self
            .storage
            .transactions
            .sum_by_category(date_from, date_to, "expense", &self.default_currency)
            .await?;

        if let Some(cat) = category {
            rows.retain(|(c, _)| c == cat);
        }

        let total: i64 = rows.iter().map(|(_, amount)| amount).sum();

        let breakdown: Vec<serde_json::Value> = rows
            .iter()
            .map(|(cat, amount)| {
                let pct = if total > 0 { amount * 100 / total } else { 0 };
                json!({ "category": cat, "amount": amount, "pct": pct })
            })
            .collect();

        let result = json!({
            "period_label": period_label,
            "base_currency": &self.default_currency,
            "total": total,
            "breakdown": breakdown,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    // ── report_income ────────────────────────────────────────────────────────

    async fn report_income(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let (date_from, date_to, period_label) = self.resolve_period_dates(p)?;
        let category = p.optional_str("category")?;

        let mut rows = self
            .storage
            .transactions
            .sum_by_category(date_from, date_to, "income", &self.default_currency)
            .await?;

        if let Some(cat) = category {
            rows.retain(|(c, _)| c == cat);
        }

        let total: i64 = rows.iter().map(|(_, amount)| amount).sum();

        let breakdown: Vec<serde_json::Value> = rows
            .iter()
            .map(|(cat, amount)| {
                let pct = if total > 0 { amount * 100 / total } else { 0 };
                json!({ "category": cat, "amount": amount, "pct": pct })
            })
            .collect();

        let result = json!({
            "period_label": period_label,
            "base_currency": &self.default_currency,
            "total": total,
            "breakdown": breakdown,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    // ── report_trends ────────────────────────────────────────────────────────

    async fn report_trends(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let metric = p.required_str("metric")?;
        let periods = p.i64_or("periods", 6)?.clamp(1, 120) as i32;

        let data: Vec<serde_json::Value> = if metric == "savings_rate" {
            // Fetch both income and spending time series concurrently.
            let (income_data, spend_data) = tokio::try_join!(
                self.storage.transactions.sum_by_period(
                    "income",
                    periods,
                    "month",
                    &self.default_currency
                ),
                self.storage.transactions.sum_by_period(
                    "expense",
                    periods,
                    "month",
                    &self.default_currency
                ),
            )?;

            let income_map: std::collections::HashMap<String, i64> =
                income_data.into_iter().collect();
            let spend_map: std::collections::HashMap<String, i64> =
                spend_data.into_iter().collect();

            // Union of period labels, sorted descending (most recent first).
            let mut all_labels: Vec<String> =
                income_map.keys().chain(spend_map.keys()).cloned().collect();
            all_labels.sort_unstable();
            all_labels.dedup();
            all_labels.reverse();

            let values: Vec<i64> = all_labels
                .iter()
                .map(|label| {
                    let income = income_map.get(label).copied().unwrap_or(0);
                    let spending = spend_map.get(label).copied().unwrap_or(0);
                    if income > 0 {
                        (income - spending) * 100 / income
                    } else {
                        0
                    }
                })
                .collect();

            all_labels
                .iter()
                .zip(values.iter())
                .enumerate()
                .map(|(i, (label, &value))| {
                    let chg = if i + 1 < values.len() {
                        change_pct(value, values[i + 1])
                    } else {
                        None
                    };
                    json!({ "period_label": label, "value": value, "change_pct": chg })
                })
                .collect()
        } else {
            let tx_type = if metric == "spending" {
                "expense"
            } else {
                "income"
            };

            let period_data = self
                .storage
                .transactions
                .sum_by_period(tx_type, periods, "month", &self.default_currency)
                .await?;

            let values: Vec<i64> = period_data.iter().map(|(_, v)| *v).collect();

            period_data
                .iter()
                .enumerate()
                .map(|(i, (label, value))| {
                    let chg = if i + 1 < values.len() {
                        change_pct(*value, values[i + 1])
                    } else {
                        None
                    };
                    json!({ "period_label": label, "value": value, "change_pct": chg })
                })
                .collect()
        };

        let result = json!({ "metric": metric, "data": data });
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    // ── report_net_worth_history ─────────────────────────────────────────────

    async fn report_net_worth_history(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let today = Local::now().date_naive();
        let base = &self.default_currency;

        let (accounts_total, investments_total, liabilities_total) = tokio::try_join!(
            self.storage.accounts.total_base_balance(base),
            self.storage.investments.total_base_value(base),
            self.storage.liabilities.total_base_remaining(base),
        )?;

        let net_worth = accounts_total + investments_total - liabilities_total;

        let result = json!({
            "base_currency": base,
            "data": [{
                "date": today.to_string(),
                "net_worth": net_worth,
                "accounts": accounts_total,
                "investments": investments_total,
                "liabilities": liabilities_total,
            }],
            "note": "Current snapshot only. Historical snapshots are not yet available.",
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    // ── daily_review ─────────────────────────────────────────────────────────

    async fn daily_review_action(&self) -> Result<String> {
        if let Some(handler) = &self.finance_handler {
            handler.daily_review().await
        } else {
            Ok(
                "Finance handler not configured. Daily review requires a running finance handler."
                    .to_string(),
            )
        }
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Resolve (date_from, date_to, period_label) from the request parameters.
    ///
    /// If both `date_from` and `date_to` are present they take precedence over
    /// the `period` keyword; otherwise `period` is required and used to derive
    /// the date range.
    fn resolve_period_dates(
        &self,
        p: &ParamExtractor<'_>,
    ) -> Result<(NaiveDate, NaiveDate, String)> {
        let date_from_str = p.optional_str("date_from")?;
        let date_to_str = p.optional_str("date_to")?;

        if let (Some(from), Some(to)) = (date_from_str, date_to_str) {
            let date_from = parse_date(from)?;
            let date_to = parse_date(to)?;
            let label = format!("{date_from} to {date_to}");
            return Ok((date_from, date_to, label));
        }

        let today = Local::now().date_naive();
        let period = p.str_or("period", "monthly")?;
        derive_date_range(period, today)
    }
}

#[cfg(test)]
mod tests {
    use super::{change_pct, derive_date_range};
    use chrono::NaiveDate;

    #[test]
    fn test_derive_date_range_month() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let (from, to, label) = derive_date_range("month", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
        assert_eq!(label, "February 2026");
    }

    #[test]
    fn test_derive_date_range_monthly_alias() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let (from, to, _) = derive_date_range("monthly", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }

    #[test]
    fn test_derive_date_range_week() {
        // 2026-02-15 is a Sunday
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let (from, to, label) = derive_date_range("week", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 2, 9).unwrap()); // Monday
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()); // Sunday
        assert!(label.starts_with("Week of"));
    }

    #[test]
    fn test_derive_date_range_year() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let (from, to, label) = derive_date_range("year", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
        assert_eq!(label, "2026");
    }

    #[test]
    fn test_derive_date_range_quarter() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let (from, to, label) = derive_date_range("quarter", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
        assert_eq!(label, "Q2 2026");
    }

    #[test]
    fn test_derive_date_range_last_30_days() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let (from, _to, _) = derive_date_range("last_30_days", today).unwrap();
        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 1, 31).unwrap());
    }

    #[test]
    fn test_derive_date_range_unknown_returns_error() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        assert!(derive_date_range("banana", today).is_err());
    }

    #[test]
    fn test_derive_date_range_custom_returns_error() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        assert!(derive_date_range("custom", today).is_err());
    }

    #[test]
    fn test_change_pct() {
        assert_eq!(change_pct(200, 100), Some(100)); // +100%
        assert_eq!(change_pct(50, 100), Some(-50)); // -50%
        assert_eq!(change_pct(100, 0), None); // div by zero
        assert_eq!(change_pct(100, 100), Some(0)); // no change
    }
}
