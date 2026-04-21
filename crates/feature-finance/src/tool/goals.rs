//! Goal, liability, and net-worth action handlers for `FinanceTool`.
//!
//! Handles: goal_create, goal_list, goal_update, goal_fire, goal_whatif,
//! liability_add, liability_list, liability_update, net_worth.

use jiff::{civil::Date, Timestamp, Zoned};
use serde_json::json;

use crate::currency::ensure_base_amount;
use crate::types::{FinanceGoal, FinanceLiability, GoalStatus, GoalType, LiabilityType};
use common::{Result, ToolError};
use storage::rows::finance::{
    FinanceGoalPatch, FinanceGoalRow, FinanceLiabilityPatch, FinanceLiabilityRow,
};
use storage::StorageError;
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use super::{parse_date, FinanceTool};

impl FinanceTool {
    pub(crate) async fn handle_goal(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "goal_create" => self.goal_create(p).await,
            "goal_list" => self.goal_list(p).await,
            "goal_update" => self.goal_update(p).await,
            "goal_delete" => self.goal_delete(p).await,
            "goal_fire" => self.goal_fire(p).await,
            "goal_whatif" => self.goal_whatif(p).await,
            "liability_add" => self.liability_add(p).await,
            "liability_list" => self.liability_list().await,
            "liability_update" => self.liability_update(p).await,
            "liability_delete" => self.liability_delete(p).await,
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
        let deadline = p.optional_str("deadline")?.map(parse_date).transpose()?;

        if let Some(ref d) = deadline {
            let today = Zoned::now().date();
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

        let now = Timestamp::now();
        let id = uuid::Uuid::new_v4().to_string();

        let conv = ensure_base_amount(
            target_amount,
            currency,
            &self.default_currency,
            &self.price_service,
        )
        .await?;
        let base_current_amount = (current_amount as f64 * conv.exchange_rate).round() as i64;

        let row = FinanceGoalRow {
            id,
            name: name.to_string(),
            goal_type: goal_type.as_str().to_string(),
            target_amount,
            current_amount,
            currency: currency.to_uppercase(),
            status: "active".to_string(),
            deadline: deadline.map(|d| d.into()),
            monthly_contribution,
            expected_return_rate,
            inflation_rate,
            notes: notes.map(|s| s.to_string()),
            created_at: now.into(),
            updated_at: now.into(),
            base_target_amount: conv.base_amount,
            base_current_amount,
            base_currency: conv.base_currency,
            exchange_rate: conv.exchange_rate,
        };

        let inserted = self.storage.goals.add(&row).await?;

        if let Some(ref bus) = self.domain_bus {
            bus.publish(crate::events::FinanceEvent::GoalCreated {
                goal_id: inserted.id.clone(),
                name: inserted.name.clone(),
                target_amount: inserted.target_amount,
            }.into());
        }

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

    async fn goal_list(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let status = p.optional_str("goal_status")?;
        let rows = match status {
            Some("all") => self.storage.goals.list_all().await?,
            _ => self.storage.goals.list_active().await?,
        };

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
        let name = p.optional_str("name")?.map(String::from);
        let current_amount = p.optional_i64("current_amount")?;
        let target_amount = p.optional_i64("target_amount")?;
        let monthly_contribution = p.optional_i64("monthly_contribution")?;
        let expected_return_rate = p.optional_f64("expected_return_rate")?;
        let inflation_rate = p.optional_f64("inflation_rate")?;
        let deadline = p.optional_str("deadline")?.map(parse_date).transpose()?;
        let status = p
            .optional_str("status")?
            .map(|s| {
                GoalStatus::from_str_loose(s)
                    .ok_or_else(|| ToolError::InvalidParams(format!("Invalid status: {s}")))
            })
            .transpose()?;

        if name.is_none()
            && current_amount.is_none()
            && target_amount.is_none()
            && monthly_contribution.is_none()
            && expected_return_rate.is_none()
            && inflation_rate.is_none()
            && deadline.is_none()
            && status.is_none()
        {
            return Err(ToolError::InvalidParams("No fields to update".to_string()).into());
        }

        let patch = FinanceGoalPatch {
            id: id.to_string(),
            name,
            current_amount,
            target_amount,
            monthly_contribution: monthly_contribution.map(Some),
            expected_return_rate: expected_return_rate.map(Some),
            inflation_rate: inflation_rate.map(Some),
            deadline: deadline.map(|d| Some(d.into())),
            status: status.map(|s| s.as_str().to_string()),
            base_target_amount: None,
            base_current_amount: None,
            base_currency: None,
            exchange_rate: None,
        };

        let old_row = self.storage.goals.get(id).await?;

        let row = self
            .storage
            .goals
            .update(&patch)
            .await
            .map_err(|e| match e {
                StorageError::NotFound(_) => {
                    ToolError::ExecutionFailed(format!("Goal {id} not found"))
                }
                other => ToolError::ExecutionFailed(other.to_string()),
            })?;

        let goal = FinanceGoal::from(row);

        // Emit GoalAchieved when current_amount first reaches or exceeds target_amount.
        if let Some(ref bus) = self.domain_bus {
            let was_achieved = old_row.map(|r| r.current_amount >= r.target_amount).unwrap_or(false);
            if !was_achieved && goal.current_amount >= goal.target_amount {
                bus.publish(crate::events::FinanceEvent::GoalAchieved {
                    goal_id: goal.id.clone(),
                    name: goal.name.clone(),
                }.into());
            }
        }
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

    async fn goal_delete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let deleted = self.storage.goals.delete(id).await?;
        if deleted {
            Ok(format!("Goal {id} deleted."))
        } else {
            Err(ToolError::InvalidParams(format!("Goal not found: {id}")).into())
        }
    }

    async fn goal_fire(&self, p: &ParamExtractor<'_>) -> Result<String> {
        self.fire_calculation(p, false).await
    }

    async fn goal_whatif(&self, p: &ParamExtractor<'_>) -> Result<String> {
        self.fire_calculation(p, true).await
    }

    async fn fire_calculation(&self, p: &ParamExtractor<'_>, whatif: bool) -> Result<String> {
        let withdrawal_rate = p.optional_f64("withdrawal_rate")?.unwrap_or(4.0);
        let expected_return = p.optional_f64("expected_return_rate")?;
        let inflation_rate = p.optional_f64("inflation_rate")?;
        let monthly_contribution = p.optional_i64("monthly_contribution")?.unwrap_or(0);

        let annual_expenses: i64 = match p.optional_i64("annual_expenses")? {
            Some(v) => v,
            None => {
                let today = Zoned::now().date();
                let date_from = Date::new(today.year() - 1, today.month(), today.day())
                    .unwrap_or_else(|_| today.checked_sub(jiff::Span::new().days(365)).unwrap());
                let cats = self
                    .storage
                    .transactions
                    .sum_by_category(date_from, today, "expense", &self.default_currency)
                    .await?;
                cats.iter().map(|(_, total)| total).sum()
            }
        };

        if annual_expenses <= 0 {
            return Err(ToolError::InvalidParams(
                "annual_expenses must be positive (or no expense transactions found)".to_string(),
            )
            .into());
        }

        let base = &self.default_currency;
        let (accounts_total, investments_total, liabilities_total) = tokio::try_join!(
            self.storage.accounts.total_base_balance(base),
            self.storage.investments.total_base_value(base),
            self.storage.liabilities.total_base_remaining(base),
        )?;

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

        let today = Zoned::now().date();
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
            let remaining = (fire_number - current_net_worth) as f64;
            return Some(remaining / monthly_savings as f64);
        }

        let pmt_r = monthly_savings as f64 / r;
        let numerator = fire_number as f64 + pmt_r;
        let denominator = current_net_worth as f64 + pmt_r;

        if denominator <= 0.0 || numerator / denominator <= 0.0 {
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
        let due_date = p.optional_str("due_date")?.map(parse_date).transpose()?;
        let notes = p.optional_str("notes")?;

        let now = Timestamp::now();
        let id = uuid::Uuid::new_v4().to_string();

        let conv = ensure_base_amount(
            principal,
            currency,
            &self.default_currency,
            &self.price_service,
        )
        .await?;
        let base_remaining = (remaining as f64 * conv.exchange_rate).round() as i64;

        let row = FinanceLiabilityRow {
            id,
            name: name.to_string(),
            liability_type: liability_type.as_str().to_string(),
            principal,
            remaining,
            currency: currency.to_uppercase(),
            interest_rate,
            monthly_payment,
            due_date: due_date.map(|d| d.into()),
            notes: notes.map(|s| s.to_string()),
            created_at: now.into(),
            updated_at: now.into(),
            base_principal: conv.base_amount,
            base_remaining,
            base_currency: conv.base_currency,
            exchange_rate: conv.exchange_rate,
        };

        let inserted = self.storage.liabilities.add(&row).await?;

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
        let rows = self.storage.liabilities.list_all().await?;

        let totals = self
            .storage
            .liabilities
            .total_remaining_by_currency()
            .await?;

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
            base_principal: None,
            base_remaining: None,
            base_currency: None,
            exchange_rate: None,
        };

        let row = self
            .storage
            .liabilities
            .update(&patch)
            .await
            .map_err(|e| match e {
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

    async fn liability_delete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let deleted = self.storage.liabilities.delete(id).await?;
        if deleted {
            Ok(format!("Liability {id} deleted."))
        } else {
            Err(ToolError::InvalidParams(format!("Liability not found: {id}")).into())
        }
    }

    async fn net_worth(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let base = &self.default_currency;

        let (accounts_total, investments_total, liabilities_total) = tokio::try_join!(
            self.storage.accounts.total_base_balance(base),
            self.storage.investments.total_base_value(base),
            self.storage.liabilities.total_base_remaining(base),
        )?;

        let net_worth = accounts_total + investments_total - liabilities_total;

        Ok(serde_json::to_string_pretty(&json!({
            "base_currency": base,
            "net_worth": net_worth,
            "accounts": accounts_total,
            "investments": investments_total,
            "liabilities": liabilities_total,
        }))
        .unwrap())
    }
}

fn fire_date_label(from: Date, months: f64) -> String {
    let total = months as i64;
    let extra_years = total / 12;
    let extra_months = total % 12;
    let new_month0 = (from.month() as i64 - 1) + extra_months;
    let year = from.year() as i64 + extra_years + new_month0 / 12;
    let month = (new_month0 % 12 + 1) as i8;
    format!("{year}-{month:02}")
}
