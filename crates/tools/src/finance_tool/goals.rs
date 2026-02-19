//! Goal, liability, and net-worth action handlers for `FinanceTool`.
//!
//! Handles: goal_create, goal_list, goal_update, goal_fire, goal_whatif,
//! liability_add, liability_list, liability_update, net_worth.

use chrono::{Datelike, NaiveDate, Utc};
use serde_json::json;

use crate::finance_types::{FinanceGoal, FinanceLiability, GoalStatus, GoalType, LiabilityType};
use crate::params::ParamExtractor;
use crate::RoutingContext;
use common::{Result, ToolError};
use storage::{
    FinanceGoalPatch, FinanceGoalRow, FinanceLiabilityPatch, FinanceLiabilityRow, StorageError,
};

use super::FinanceTool;

impl FinanceTool {
    pub(crate) async fn handle_goal(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "goal_create" => self.goal_create(p).await,
            "goal_list" => self.goal_list().await,
            "goal_update" => self.goal_update(p).await,
            "goal_fire" => self.goal_fire(p).await,
            "goal_whatif" => self.goal_whatif(p).await,
            "liability_add" => self.liability_add(p).await,
            "liability_list" => self.liability_list().await,
            "liability_update" => self.liability_update(p).await,
            "net_worth" => self.net_worth(p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown goal action: {action}")).into()),
        }
    }

    async fn goal_create(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let name = p.required_str("name")?;
        let goal_type_str = p.required_str("goal_type")?;
        let goal_type = GoalType::from_str_loose(goal_type_str).ok_or_else(|| {
            ToolError::InvalidParams(format!("Invalid goal_type: {goal_type_str}"))
        })?;
        let target_amount = p.required_i64("target_amount")?;
        if target_amount <= 0 {
            return Err(
                ToolError::InvalidParams("Target amount must be positive".to_string()).into(),
            );
        }
        let currency = p
            .optional_str("currency")?
            .unwrap_or(&self.default_currency);
        let current_amount = p.i64_or("current_amount", 0)?;
        let deadline = p
            .optional_str("deadline")?
            .map(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map_err(|e| ToolError::InvalidParams(format!("Invalid deadline: {e}")))
            })
            .transpose()?;

        // Validate deadline is in the future
        if let Some(ref d) = deadline {
            let today = Utc::now().date_naive();
            if *d < today {
                return Err(
                    ToolError::InvalidParams("Deadline must be in the future".to_string()).into(),
                );
            }
        }
        let monthly_contribution = p.optional_i64("monthly_contribution")?;
        let expected_return_rate = p.optional_f64("expected_return_rate")?;
        let inflation_rate = p.optional_f64("inflation_rate")?;
        let notes = p.optional_str("notes")?;

        let now = Utc::now();
        let id = uuid::Uuid::new_v4().to_string();

        let row = FinanceGoalRow {
            id,
            name: name.to_string(),
            goal_type: goal_type.as_str().to_string(),
            target_amount,
            current_amount,
            currency: currency.to_uppercase(),
            status: "active".to_string(),
            deadline,
            monthly_contribution,
            expected_return_rate,
            inflation_rate,
            notes: notes.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };

        let inserted = self
            .goals
            .add(&row)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let goal = FinanceGoal::from(inserted);
        let progress_pct = if goal.target_amount > 0 {
            goal.current_amount * 100 / goal.target_amount
        } else {
            0
        };

        let result = json!({
            "id": goal.id,
            "name": goal.name,
            "goal_type": goal.goal_type.as_str(),
            "target_amount": goal.target_amount,
            "current_amount": goal.current_amount,
            "currency": goal.currency,
            "status": goal.status.as_str(),
            "deadline": goal.deadline,
            "monthly_contribution": goal.monthly_contribution,
            "expected_return_rate": goal.expected_return_rate,
            "inflation_rate": goal.inflation_rate,
            "notes": goal.notes,
            "progress_pct": progress_pct,
            "created_at": goal.created_at,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn goal_list(&self) -> Result<String> {
        let rows = self
            .goals
            .list_active()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if rows.is_empty() {
            return Ok("No active goals.".to_string());
        }

        let goals: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|row| {
                let goal = FinanceGoal::from(row);
                let progress_pct = if goal.target_amount > 0 {
                    goal.current_amount * 100 / goal.target_amount
                } else {
                    0
                };
                json!({
                    "id": goal.id,
                    "name": goal.name,
                    "goal_type": goal.goal_type.as_str(),
                    "target_amount": goal.target_amount,
                    "current_amount": goal.current_amount,
                    "currency": goal.currency,
                    "status": goal.status.as_str(),
                    "deadline": goal.deadline,
                    "monthly_contribution": goal.monthly_contribution,
                    "expected_return_rate": goal.expected_return_rate,
                    "progress_pct": progress_pct,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&json!({ "goals": goals })).unwrap())
    }

    async fn goal_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let current_amount = p.optional_i64("current_amount")?;
        let target_amount = p.optional_i64("target_amount")?;
        let monthly_contribution = p.optional_i64("monthly_contribution")?;
        let expected_return_rate = p.optional_f64("expected_return_rate")?;
        let deadline = p
            .optional_str("deadline")?
            .map(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map_err(|e| ToolError::InvalidParams(format!("Invalid deadline: {e}")))
            })
            .transpose()?;
        let status = p
            .optional_str("status")?
            .map(|s| {
                GoalStatus::from_str_loose(s)
                    .ok_or_else(|| ToolError::InvalidParams(format!("Invalid status: {s}")))
            })
            .transpose()?;

        if current_amount.is_none()
            && target_amount.is_none()
            && monthly_contribution.is_none()
            && expected_return_rate.is_none()
            && deadline.is_none()
            && status.is_none()
        {
            return Err(ToolError::InvalidParams("No fields to update".to_string()).into());
        }

        let patch = FinanceGoalPatch {
            id: id.to_string(),
            name: None,
            current_amount,
            target_amount,
            monthly_contribution: monthly_contribution.map(Some),
            expected_return_rate: expected_return_rate.map(Some),
            inflation_rate: None,
            deadline: deadline.map(Some),
            status: status.map(|s| s.as_str().to_string()),
        };

        let row = self.goals.update(&patch).await.map_err(|e| match e {
            StorageError::NotFound(_) => ToolError::ExecutionFailed(format!("Goal {id} not found")),
            other => ToolError::ExecutionFailed(other.to_string()),
        })?;

        let goal = FinanceGoal::from(row);
        let progress_pct = if goal.target_amount > 0 {
            goal.current_amount * 100 / goal.target_amount
        } else {
            0
        };

        let result = json!({
            "id": goal.id,
            "name": goal.name,
            "goal_type": goal.goal_type.as_str(),
            "target_amount": goal.target_amount,
            "current_amount": goal.current_amount,
            "currency": goal.currency,
            "status": goal.status.as_str(),
            "deadline": goal.deadline,
            "progress_pct": progress_pct,
            "updated_at": goal.updated_at,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn goal_fire(&self, p: &ParamExtractor<'_>) -> Result<String> {
        self.fire_calculation(p, false).await
    }

    async fn goal_whatif(&self, p: &ParamExtractor<'_>) -> Result<String> {
        self.fire_calculation(p, true).await
    }

    /// Core FIRE / what-if calculation. When `whatif` is true, an adjusted
    /// scenario is also computed using `extra_monthly_savings` and/or
    /// `extra_return_rate` and appended to the result.
    async fn fire_calculation(&self, p: &ParamExtractor<'_>, whatif: bool) -> Result<String> {
        let withdrawal_rate = p.optional_f64("withdrawal_rate")?.unwrap_or(4.0);
        let expected_return = p.optional_f64("expected_return_rate")?;
        let inflation_rate = p.optional_f64("inflation_rate")?;
        let monthly_contribution = p.optional_i64("monthly_contribution")?.unwrap_or(0);

        // Annual expenses — use explicit param or derive from last 12 months.
        let annual_expenses: i64 = match p.optional_i64("annual_expenses")? {
            Some(v) => v,
            None => {
                let today = Utc::now().date_naive();
                let date_from =
                    NaiveDate::from_ymd_opt(today.year() - 1, today.month(), today.day())
                        .unwrap_or_else(|| today - chrono::Duration::days(365));
                let cats = self
                    .transactions
                    .sum_by_category(date_from, today, "expense")
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                cats.iter().map(|(_, total)| total).sum()
            }
        };

        if annual_expenses <= 0 {
            return Err(ToolError::InvalidParams(
                "annual_expenses must be positive (or no expense transactions found)".to_string(),
            )
            .into());
        }

        // Current net worth across all currencies (summed, assumes comparable).
        let accounts_total: i64 = self
            .accounts
            .total_balance_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .iter()
            .map(|(_, v)| v)
            .sum();
        let investments_total: i64 = self
            .investments
            .total_value_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .iter()
            .map(|(_, v)| v)
            .sum();
        let liabilities_total: i64 = self
            .liabilities
            .total_remaining_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .iter()
            .map(|(_, v)| v)
            .sum();

        let current_net_worth = accounts_total + investments_total - liabilities_total;
        let fire_number = (annual_expenses as f64 * (100.0 / withdrawal_rate)) as i64;

        let progress_pct = if fire_number > 0 {
            current_net_worth * 100 / fire_number
        } else {
            0
        };

        let baseline_months = Self::months_to_fire(
            fire_number,
            current_net_worth,
            monthly_contribution,
            expected_return,
            inflation_rate,
        );

        let today = Utc::now().date_naive();
        let estimated_fire_date = baseline_months.map(|m| fire_date_label(today, m));

        let mut result = json!({
            "fire_number": fire_number,
            "current_net_worth": current_net_worth,
            "annual_expenses": annual_expenses,
            "withdrawal_rate": withdrawal_rate,
            "progress_pct": progress_pct,
            "months_remaining": baseline_months,
            "estimated_fire_date": estimated_fire_date,
        });

        if whatif {
            let extra_monthly = p.optional_i64("extra_monthly_savings")?.unwrap_or(0);
            let extra_return = p.optional_f64("extra_return_rate")?.unwrap_or(0.0);
            let adj_monthly = monthly_contribution + extra_monthly;
            let adj_return = expected_return.map(|r| r + extra_return);

            let adjusted_months = Self::months_to_fire(
                fire_number,
                current_net_worth,
                adj_monthly,
                adj_return,
                inflation_rate,
            );

            let delta_months = match (baseline_months, adjusted_months) {
                (Some(b), Some(a)) => Some(b - a),
                _ => None,
            };

            result["whatif"] = json!({
                "extra_monthly_savings": extra_monthly,
                "extra_return_rate": extra_return,
                "adjusted_months_remaining": adjusted_months,
                "adjusted_fire_date": adjusted_months.map(|m| fire_date_label(today, m)),
                "delta_months": delta_months,
            });
        }

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    /// Compute months to reach `fire_number` from `current_net_worth` with
    /// constant `monthly_savings` and compound real monthly return `r`.
    ///
    /// Formula: FV = PV*(1+r)^n + PMT*((1+r)^n - 1)/r  →  solve for n.
    /// Falls back to the spec's simplified form when the denominator is
    /// non-positive (e.g. net worth exceeds FIRE target).
    fn months_to_fire(
        fire_number: i64,
        current_net_worth: i64,
        monthly_savings: i64,
        expected_return: Option<f64>,
        inflation_rate: Option<f64>,
    ) -> Option<f64> {
        if monthly_savings <= 0 {
            return None;
        }

        let (ret, inf) = match (expected_return, inflation_rate) {
            (Some(r), Some(i)) => (r, i),
            (Some(r), None) => (r, 0.0),
            _ => return None,
        };

        if current_net_worth >= fire_number {
            return Some(0.0);
        }

        let r = (ret - inf) / 100.0 / 12.0;

        if r <= 0.0 {
            // No compounding: simple linear projection.
            let remaining = (fire_number - current_net_worth) as f64;
            return Some(remaining / monthly_savings as f64);
        }

        // Full annuity formula: n = ln((FV + PMT/r) / (PV + PMT/r)) / ln(1+r)
        let pmt_r = monthly_savings as f64 / r;
        let numerator = fire_number as f64 + pmt_r;
        let denominator = current_net_worth as f64 + pmt_r;

        if denominator <= 0.0 || numerator / denominator <= 0.0 {
            // Fallback to spec's simplified formula (assumes PV = 0).
            let months =
                (fire_number as f64 * r / monthly_savings as f64 + 1.0).ln() / (1.0 + r).ln();
            return Some(months.max(0.0));
        }

        let months = (numerator / denominator).ln() / (1.0_f64 + r).ln();
        Some(months.max(0.0))
    }

    async fn liability_add(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let name = p.required_str("name")?;
        let type_str = p.required_str("type")?;
        let liability_type = LiabilityType::from_str_loose(type_str).ok_or_else(|| {
            ToolError::InvalidParams(format!("Invalid liability type: {type_str}"))
        })?;
        let principal = p.required_i64("principal")?;
        if principal <= 0 {
            return Err(ToolError::InvalidParams("Principal must be positive".to_string()).into());
        }
        let remaining = p.i64_or("remaining", principal)?;
        if remaining <= 0 {
            return Err(ToolError::InvalidParams("Remaining must be positive".to_string()).into());
        }
        let currency = p
            .optional_str("currency")?
            .unwrap_or(&self.default_currency);
        let interest_rate = p.optional_f64("interest_rate")?;
        let monthly_payment = p.optional_i64("monthly_payment")?;
        let due_date = p
            .optional_str("due_date")?
            .map(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map_err(|e| ToolError::InvalidParams(format!("Invalid due_date: {e}")))
            })
            .transpose()?;
        let notes = p.optional_str("notes")?;

        let now = Utc::now();
        let id = uuid::Uuid::new_v4().to_string();

        let row = FinanceLiabilityRow {
            id,
            name: name.to_string(),
            liability_type: liability_type.as_str().to_string(),
            principal,
            remaining,
            currency: currency.to_uppercase(),
            interest_rate,
            monthly_payment,
            due_date,
            notes: notes.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };

        let inserted = self
            .liabilities
            .add(&row)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let l = FinanceLiability::from(inserted);
        let result = json!({
            "id": l.id,
            "name": l.name,
            "liability_type": l.liability_type.as_str(),
            "principal": l.principal,
            "remaining": l.remaining,
            "currency": l.currency,
            "interest_rate": l.interest_rate,
            "monthly_payment": l.monthly_payment,
            "due_date": l.due_date,
            "notes": l.notes,
            "created_at": l.created_at,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn liability_list(&self) -> Result<String> {
        let rows = self
            .liabilities
            .list_all()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let totals = self
            .liabilities
            .total_remaining_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let liabilities: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|row| {
                let l = FinanceLiability::from(row);
                json!({
                    "id": l.id,
                    "name": l.name,
                    "liability_type": l.liability_type.as_str(),
                    "principal": l.principal,
                    "remaining": l.remaining,
                    "currency": l.currency,
                    "interest_rate": l.interest_rate,
                    "monthly_payment": l.monthly_payment,
                    "due_date": l.due_date,
                    "notes": l.notes,
                })
            })
            .collect();

        let total_map: serde_json::Map<String, serde_json::Value> = totals
            .into_iter()
            .map(|(cur, total)| (cur, json!(total)))
            .collect();

        Ok(serde_json::to_string_pretty(&json!({
            "liabilities": liabilities,
            "total_remaining_by_currency": serde_json::Value::Object(total_map),
        }))
        .unwrap())
    }

    async fn liability_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let remaining = p.optional_i64("remaining")?;
        let monthly_payment = p.optional_i64("monthly_payment")?;
        let interest_rate = p.optional_f64("interest_rate")?;
        let notes = p.optional_str("notes")?;

        if remaining.is_none()
            && monthly_payment.is_none()
            && interest_rate.is_none()
            && notes.is_none()
        {
            return Err(ToolError::InvalidParams("No fields to update".to_string()).into());
        }

        let patch = FinanceLiabilityPatch {
            id: id.to_string(),
            remaining,
            monthly_payment: monthly_payment.map(Some),
            interest_rate: interest_rate.map(Some),
            notes: notes.map(|s| Some(s.to_string())),
        };

        let row = self.liabilities.update(&patch).await.map_err(|e| match e {
            StorageError::NotFound(_) => {
                ToolError::ExecutionFailed(format!("Liability {id} not found"))
            }
            other => ToolError::ExecutionFailed(other.to_string()),
        })?;

        let l = FinanceLiability::from(row);
        let result = json!({
            "id": l.id,
            "name": l.name,
            "liability_type": l.liability_type.as_str(),
            "principal": l.principal,
            "remaining": l.remaining,
            "currency": l.currency,
            "interest_rate": l.interest_rate,
            "monthly_payment": l.monthly_payment,
            "due_date": l.due_date,
            "notes": l.notes,
            "updated_at": l.updated_at,
        });

        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn net_worth(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let accounts = self
            .accounts
            .total_balance_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let investments = self
            .investments
            .total_value_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let liabilities = self
            .liabilities
            .total_remaining_by_currency()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        // Aggregate per-currency totals.
        let mut currencies: std::collections::BTreeMap<String, [i64; 3]> =
            std::collections::BTreeMap::new();
        for (cur, val) in &accounts {
            currencies.entry(cur.clone()).or_default()[0] += val;
        }
        for (cur, val) in &investments {
            currencies.entry(cur.clone()).or_default()[1] += val;
        }
        for (cur, val) in &liabilities {
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

        let total_net_worth: i64 = currencies
            .values()
            .map(|totals| totals[0] + totals[1] - totals[2])
            .sum();

        Ok(serde_json::to_string_pretty(&json!({
            "net_worth": total_net_worth,
            "breakdown": serde_json::Value::Object(breakdown),
        }))
        .unwrap())
    }
}

/// Convert a fractional months value into a "YYYY-MM" label relative to `from`.
fn fire_date_label(from: NaiveDate, months: f64) -> String {
    let total = months as i64;
    let extra_years = total / 12;
    let extra_months = total % 12;
    let new_month0 = from.month0() as i64 + extra_months;
    let year = from.year() as i64 + extra_years + new_month0 / 12;
    let month = (new_month0 % 12 + 1) as u32;
    format!("{year}-{month:02}")
}

#[cfg(test)]
mod tests {
    use crate::finance_types::{GoalStatus, GoalType, LiabilityType};

    #[test]
    fn test_goal_type_from_str_loose() {
        assert!(GoalType::from_str_loose("savings").is_some());
        assert!(GoalType::from_str_loose("purchase").is_some());
        assert!(GoalType::from_str_loose("debt_payoff").is_some());
        assert!(GoalType::from_str_loose("fire").is_some());
        assert!(GoalType::from_str_loose("custom").is_some());
        assert!(GoalType::from_str_loose("invalid").is_none());
    }

    #[test]
    fn test_goal_status_from_str_loose() {
        assert!(GoalStatus::from_str_loose("active").is_some());
        assert!(GoalStatus::from_str_loose("achieved").is_some());
        assert!(GoalStatus::from_str_loose("abandoned").is_some());
        assert!(GoalStatus::from_str_loose("nope").is_none());
    }

    #[test]
    fn test_liability_type_from_str_loose() {
        assert!(LiabilityType::from_str_loose("mortgage").is_some());
        assert!(LiabilityType::from_str_loose("credit_card").is_some());
        assert!(LiabilityType::from_str_loose("personal_loan").is_some());
        assert!(LiabilityType::from_str_loose("student_loan").is_some());
        assert!(LiabilityType::from_str_loose("other").is_some());
        assert!(LiabilityType::from_str_loose("invalid").is_none());
    }
}
