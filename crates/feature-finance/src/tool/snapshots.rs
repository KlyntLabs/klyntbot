//! Net worth snapshot action handlers for `FinanceTool`.
//!
//! Handles: snapshot_record, snapshot_history.

use chrono::{Duration, Local};
use serde_json::json;

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

    /// Record a point-in-time net worth snapshot by computing current base-currency totals.
    async fn snapshot_record(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let today = Local::now().date_naive();
        let base = &self.default_currency;

        let (accounts_total, investments_total, liabilities_total) = tokio::try_join!(
            self.storage.accounts.total_base_balance(base),
            self.storage.investments.total_base_value(base),
            self.storage.liabilities.total_base_remaining(base),
        )?;

        let net_worth = accounts_total + investments_total - liabilities_total;

        let breakdown = json!({
            base: {
                "accounts": accounts_total,
                "investments": investments_total,
                "liabilities": liabilities_total,
                "net": net_worth,
            }
        });
        let breakdown_json = serde_json::to_string(&breakdown).unwrap_or_else(|_| "{}".into());

        let row = self
            .storage
            .snapshots
            .add(
                &today.to_string(),
                base,
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
            "breakdown": breakdown,
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
