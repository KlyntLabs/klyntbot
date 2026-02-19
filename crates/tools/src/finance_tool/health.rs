//! Health check action handler for `FinanceTool`.
//!
//! Validates data integrity across all finance tables and returns
//! a structured report of any issues found.

use chrono::{Duration, Local};
use serde_json::json;

use crate::RoutingContext;
use common::Result;

use super::FinanceTool;

/// Severity level for health check issues.
#[derive(Debug, Clone, Copy)]
enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// A single health check finding.
struct Issue {
    check: &'static str,
    severity: Severity,
    count: usize,
    detail: String,
}

impl FinanceTool {
    pub(crate) async fn finance_health_check(&self, _ctx: &RoutingContext) -> Result<String> {
        let mut issues: Vec<Issue> = Vec::new();

        // 1. No accounts — finance is unusable
        let accounts = self.accounts.list(false).await.unwrap_or_default();
        if accounts.is_empty() {
            issues.push(Issue {
                check: "no_accounts",
                severity: Severity::Info,
                count: 0,
                detail: "No finance accounts exist. Create an account to get started.".into(),
            });
        }

        // 2. Balance reconciliation — check for negative balances on non-crypto accounts
        for account in &accounts {
            if account.balance < 0 && account.account_type != "crypto_wallet" {
                issues.push(Issue {
                    check: "negative_balance",
                    severity: Severity::Warning,
                    count: 1,
                    detail: format!(
                        "Account '{}' has negative balance: {}",
                        account.name, account.balance
                    ),
                });
            }
        }

        // 3. Stale investment prices
        let investments = self
            .investments
            .list_with_symbols()
            .await
            .unwrap_or_default();
        let stale_threshold = Local::now() - Duration::hours(24);
        let stale_count = investments
            .iter()
            .filter(|inv| inv.updated_at < stale_threshold.to_utc())
            .count();
        if stale_count > 0 {
            issues.push(Issue {
                check: "stale_prices",
                severity: Severity::Warning,
                count: stale_count,
                detail: format!(
                    "{} investment(s) have prices older than 24 hours. Run price_refresh to update.",
                    stale_count
                ),
            });
        }

        // 4. Duplicate active budgets for same category + period
        let budgets = self.budgets.list_active().await.unwrap_or_default();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut dup_count = 0usize;
        for b in &budgets {
            let key = format!("{}:{}", b.category.as_deref().unwrap_or("*"), b.period);
            if !seen.insert(key) {
                dup_count += 1;
            }
        }
        if dup_count > 0 {
            issues.push(Issue {
                check: "duplicate_budgets",
                severity: Severity::Warning,
                count: dup_count,
                detail: format!(
                    "{} duplicate active budget(s) found for the same category + period.",
                    dup_count
                ),
            });
        }

        // 5. Overdue goals — past deadline but still active
        let goals = self.goals.list_active().await.unwrap_or_default();
        let today = Local::now().date_naive();
        let overdue_goals: Vec<_> = goals
            .iter()
            .filter(|g| g.deadline.map(|d| d < today).unwrap_or(false))
            .collect();
        if !overdue_goals.is_empty() {
            issues.push(Issue {
                check: "overdue_goals",
                severity: Severity::Info,
                count: overdue_goals.len(),
                detail: format!(
                    "{} active goal(s) are past their deadline.",
                    overdue_goals.len()
                ),
            });
        }

        // 6. Negative remaining on liabilities
        let liabilities = self.liabilities.list_all().await.unwrap_or_default();
        let neg_remaining: Vec<_> = liabilities.iter().filter(|l| l.remaining < 0).collect();
        if !neg_remaining.is_empty() {
            issues.push(Issue {
                check: "negative_remaining",
                severity: Severity::Error,
                count: neg_remaining.len(),
                detail: format!(
                    "{} liability(ies) have negative remaining balance.",
                    neg_remaining.len()
                ),
            });
        }

        // 7. Empty portfolios
        let portfolios = self.investments.list_portfolios().await.unwrap_or_default();
        let mut empty_count = 0usize;
        for p in &portfolios {
            let filter = storage::FinanceInvestmentFilter {
                portfolio_id: Some(p.id.clone()),
                ..Default::default()
            };
            let holdings = self
                .investments
                .list_investments(&filter)
                .await
                .unwrap_or_default();
            if holdings.is_empty() {
                empty_count += 1;
            }
        }
        if empty_count > 0 {
            issues.push(Issue {
                check: "empty_portfolios",
                severity: Severity::Info,
                count: empty_count,
                detail: format!("{} portfolio(s) have no investment holdings.", empty_count),
            });
        }

        // Build summary
        let errors = issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Error))
            .count();
        let warnings = issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Warning))
            .count();
        let infos = issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Info))
            .count();

        let status = if errors > 0 {
            "errors_found"
        } else if warnings > 0 {
            "warnings_found"
        } else if infos > 0 {
            "info_only"
        } else {
            "all_clear"
        };

        let issues_json: Vec<serde_json::Value> = issues
            .iter()
            .map(|i| {
                json!({
                    "check": i.check,
                    "severity": i.severity.as_str(),
                    "count": i.count,
                    "detail": i.detail,
                })
            })
            .collect();

        let result = json!({
            "status": status,
            "checks_run": 7,
            "issues": issues_json,
            "summary": format!("7 checks run: {} error(s), {} warning(s), {} info", errors, warnings, infos),
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Error.as_str(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
        assert_eq!(Severity::Info.as_str(), "info");
    }
}
