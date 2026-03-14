//! Net worth snapshot action handlers for `FinanceTool`.
//!
//! Handles: snapshot_record, snapshot_history.

use chrono::{Duration, Local};
use serde_json::json;
use std::collections::BTreeMap;

use common::{Result, ToolError};
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::{parse_date, FinanceTool};

impl FinanceTool {
    pub(crate) async fn handle_snapshot(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "snapshot_record" => self.snapshot_record(p).await,
            "snapshot_history" => self.snapshot_history(p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown snapshot action: {action}")).into()),
        }
    }

    /// Record a point-in-time net worth snapshot by computing current totals.
    async fn snapshot_record(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let today = Local::now().date_naive();

        let (account_balances, investment_values, liability_totals) = tokio::try_join!(
            self.storage.accounts.total_balance_by_currency(),
            self.storage.investments.total_value_by_currency(),
            self.storage.liabilities.total_remaining_by_currency(),
        )?;

        let accounts_total: i64 = account_balances.iter().map(|(_, v)| v).sum();
        let investments_total: i64 = investment_values.iter().map(|(_, v)| v).sum();
        let liabilities_total: i64 = liability_totals.iter().map(|(_, v)| v).sum();
        let net_worth = accounts_total + investments_total - liabilities_total;

        // Build a per-currency breakdown
        let mut currencies: BTreeMap<String, [i64; 3]> = BTreeMap::new();
        for (cur, val) in &account_balances {
            currencies.entry(cur.clone()).or_default()[0] += val;
        }
        for (cur, val) in &investment_values {
            currencies.entry(cur.clone()).or_default()[1] += val;
        }
        for (cur, val) in &liability_totals {
            currencies.entry(cur.clone()).or_default()[2] += val;
        }

        let breakdown: serde_json::Map<String, serde_json::Value> = currencies
            .iter()
            .map(|(cur, totals)| {
                let net = totals[0] + totals[1] - totals[2];
                (
                    cur.clone(),
                    json!({
                        "accounts": totals[0],
                        "investments": totals[1],
                        "liabilities": totals[2],
                        "net": net,
                    }),
                )
            })
            .collect();

        let breakdown_json = serde_json::to_string(&breakdown).unwrap_or_else(|_| "{}".into());

        let row = self
            .storage
            .snapshots
            .add(
                &today.to_string(),
                &self.default_currency,
                accounts_total,
                investments_total,
                liabilities_total,
                net_worth,
                &breakdown_json,
            )
            .await?;

        Ok(serde_json::to_string_pretty(&json!({
            "id": row.id,
            "snapshot_date": row.snapshot_date,
            "currency": row.currency,
            "accounts_total": row.accounts_total,
            "investments_total": row.investments_total,
            "liabilities_total": row.liabilities_total,
            "net_worth": row.net_worth,
            "breakdown": serde_json::Value::Object(breakdown),
        }))
        .unwrap())
    }

    /// Query snapshot history by date range.
    async fn snapshot_history(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let today = Local::now().date_naive();
        let default_start = (today - Duration::days(365)).to_string();

        let start_date = match p.optional_str("start_date")? {
            Some(s) => {
                // Validate the date format
                let _ = parse_date(s)?;
                s.to_string()
            }
            None => default_start,
        };

        let end_date = match p.optional_str("end_date")? {
            Some(s) => {
                let _ = parse_date(s)?;
                s.to_string()
            }
            None => today.to_string(),
        };

        let currency = p
            .optional_str("currency")?
            .unwrap_or(&self.default_currency)
            .to_string();

        let rows = self
            .storage
            .snapshots
            .list_by_date_range(&start_date, &end_date, &currency)
            .await?;

        if rows.is_empty() {
            return Ok("No snapshots found for the specified period.".to_string());
        }

        let snapshots: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "snapshot_date": row.snapshot_date,
                    "currency": row.currency,
                    "accounts_total": row.accounts_total,
                    "investments_total": row.investments_total,
                    "liabilities_total": row.liabilities_total,
                    "net_worth": row.net_worth,
                })
            })
            .collect();

        // Compute change from first to last
        let first = &rows[0];
        let last = &rows[rows.len() - 1];
        let change = last.net_worth - first.net_worth;
        let change_pct = if first.net_worth != 0 {
            (change * 100) / first.net_worth
        } else {
            0
        };

        Ok(serde_json::to_string_pretty(&json!({
            "snapshots": snapshots,
            "count": rows.len(),
            "period": {
                "start": start_date,
                "end": end_date,
            },
            "summary": {
                "first_net_worth": first.net_worth,
                "latest_net_worth": last.net_worth,
                "change": change,
                "change_pct": change_pct,
            },
        }))
        .unwrap())
    }
}
