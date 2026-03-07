# Finance Feature Complete Upgrade — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the finance feature from read-only display to full CRUD with modern glassmorphism UI matching the recently upgraded Productivity/Notes pages.

**Architecture:** Three phases — (1) Backend: add mutation Tauri commands + fix existing commands, (2) Shared components: build upgraded finance UI primitives, (3) Page rebuilds: rewrite all 7 finance pages + add Reports page. Each phase can be committed independently.

**Tech Stack:** Rust (Tauri 2 + SQLite via sqlx), React 18 + TypeScript, Tailwind v4, Radix UI primitives, Lucide icons.

---

## Phase 1: Backend — Mutation Commands + Fixes

### Task 1: Add Finance EntityKind variant

**Files:**
- Modify: `crates/desktop-shared/src/types.rs:48-75` (EntityKind enum + parse)

**Step 1: Add the Finance variant to EntityKind**

In `crates/desktop-shared/src/types.rs`, add `Finance` to the enum and its parse match:

```rust
pub enum EntityKind {
    Task,
    Project,
    Objective,
    Area,
    KeyResult,
    FocusSession,
    Productivity,
    Note,
    Notebook,
    Finance,  // ← add
}

// In parse():
"finance" | "finance_account" | "finance_transaction" | "finance_budget"
    | "finance_goal" | "finance_liability" | "finance_portfolio"
    | "finance_investment" => Some(Self::Finance),
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`

**Step 3: Commit**

```
feat(desktop-shared): add Finance variant to EntityKind
```

---

### Task 2: Add finance IPC param/response types to desktop-shared

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs:147-187` (add new types after existing finance types)

**Step 1: Add all finance param types**

Append after the existing `CurrencyNetWorth` impl block in `commands.rs`:

```rust
// ── Finance Mutation Params ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAccountCreateParams {
    pub name: String,
    pub account_type: String,
    pub currency: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAccountUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub is_archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionCreateParams {
    pub account_id: String,
    pub tx_type: String,
    pub amount: i64,
    pub currency: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub counterparty: Option<String>,
    pub tx_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionUpdateParams {
    pub id: String,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub subcategory: Option<Option<String>>,
    pub counterparty: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub tx_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetCreateParams {
    pub name: String,
    pub amount: i64,
    pub period: String,
    pub currency: Option<String>,
    pub category: Option<String>,
    pub method: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub alert_threshold: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceGoalCreateParams {
    pub name: String,
    pub goal_type: String,
    pub target_amount: i64,
    pub currency: Option<String>,
    pub current_amount: Option<i64>,
    pub deadline: Option<String>,
    pub monthly_contribution: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceGoalUpdateParams {
    pub id: String,
    pub current_amount: Option<i64>,
    pub target_amount: Option<i64>,
    pub monthly_contribution: Option<Option<i64>>,
    pub deadline: Option<Option<String>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLiabilityCreateParams {
    pub name: String,
    pub liability_type: String,
    pub principal: i64,
    pub currency: Option<String>,
    pub remaining: Option<i64>,
    pub interest_rate: Option<f64>,
    pub monthly_payment: Option<i64>,
    pub due_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLiabilityUpdateParams {
    pub id: String,
    pub remaining: Option<i64>,
    pub monthly_payment: Option<Option<i64>>,
    pub interest_rate: Option<Option<f64>>,
    pub notes: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePortfolioCreateParams {
    pub name: String,
    pub description: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentCreateParams {
    pub portfolio_id: String,
    pub asset_type: String,
    pub cost_basis: i64,
    pub quantity: f64,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub currency: Option<String>,
    pub purchase_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentUpdateParams {
    pub id: String,
    pub current_price: Option<Option<i64>>,
    pub current_value: Option<Option<i64>>,
    pub quantity: Option<f64>,
    pub notes: Option<Option<String>>,
}

// ── Finance Filter Params (for upgraded queries) ───────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionFilterParams {
    pub account_id: Option<String>,
    pub tx_type: Option<String>,
    pub category: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub query: Option<String>,
    pub limit: Option<i64>,
}

// ── Finance Report Responses ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceSpendingReportResponse {
    pub total: i64,
    pub breakdown: Vec<FinanceCategoryBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryBreakdown {
    pub category: String,
    pub amount: i64,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTrendPoint {
    pub period: String,
    pub value: i64,
    pub change_pct: Option<f64>,
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`

**Step 3: Commit**

```
feat(desktop-shared): add finance mutation param and report response types
```

---

### Task 3: Add AppCore finance mutation handlers

**Files:**
- Modify: `crates/app-core/src/handlers/finance.rs` (extend the `impl AppCore` block)

**Step 1: Add all mutation methods**

Add these methods inside the existing `impl AppCore` block, after the `finance_exchange_rates` method:

```rust
    // ── Mutations ───────────────────────────────────────────

    pub async fn finance_account_create(
        &self,
        params: FinanceAccountCreateParams,
    ) -> HandlerResult<FinanceAccountRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let currency = params.currency.unwrap_or_else(|| "VND".to_string());

        let row = FinanceAccountRow {
            id: id.clone(),
            name: params.name,
            account_type: params.account_type,
            currency,
            balance: params.balance.unwrap_or(0),
            institution: params.institution,
            notes: params.notes,
            is_archived: false,
            created_at: now,
            updated_at: now,
        };

        self.repos.finance.accounts.add(&row).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((row, updates))
    }

    pub async fn finance_account_update(
        &self,
        params: FinanceAccountUpdateParams,
    ) -> HandlerResult<FinanceAccountRow> {
        let patch = FinanceAccountPatch {
            id: params.id.clone(),
            name: params.name,
            balance: params.balance,
            institution: params.institution,
            notes: params.notes,
            is_archived: params.is_archived,
        };
        self.repos.finance.accounts.update(&patch).await.map_err(map_storage_err)?;
        let row = self.repos.finance.accounts.get(&params.id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id: params.id }];
        Ok((row, updates))
    }

    pub async fn finance_account_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos.finance.accounts.delete(&id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((true, updates))
    }

    pub async fn finance_transaction_create(
        &self,
        params: FinanceTransactionCreateParams,
    ) -> HandlerResult<FinanceTransactionRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let tx_date = params.tx_date
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .unwrap_or_else(|| now.date_naive());

        // Get account to determine currency if not provided
        let account = self.repos.finance.accounts.get(&params.account_id).await.map_err(map_storage_err)?;
        let currency = params.currency.unwrap_or(account.currency.clone());

        let row = FinanceTransactionRow {
            id: id.clone(),
            account_id: params.account_id.clone(),
            tx_type: params.tx_type.clone(),
            amount: params.amount,
            currency,
            category: params.category,
            subcategory: params.subcategory,
            counterparty: params.counterparty,
            notes: params.notes,
            tx_date,
            transfer_id: None,
            is_recurring: false,
            recurring_rule: None,
            created_at: now,
            updated_at: now,
        };

        self.repos.finance.transactions.add(&row).await.map_err(map_storage_err)?;

        // Adjust account balance
        let delta = match params.tx_type.as_str() {
            "income" => params.amount,
            "expense" => -params.amount,
            _ => 0,
        };
        if delta != 0 {
            self.repos.finance.accounts.adjust_balance(&params.account_id, delta).await.map_err(map_storage_err)?;
        }

        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((row, updates))
    }

    pub async fn finance_transaction_delete(&self, id: String) -> HandlerResult<bool> {
        // Get the transaction first so we can reverse the balance
        let tx = self.repos.finance.transactions.get(&id).await.map_err(map_storage_err)?;
        self.repos.finance.transactions.delete(&id).await.map_err(map_storage_err)?;

        // Reverse the balance adjustment
        let delta = match tx.tx_type.as_str() {
            "income" => -tx.amount,
            "expense" => tx.amount,
            _ => 0,
        };
        if delta != 0 {
            self.repos.finance.accounts.adjust_balance(&tx.account_id, delta).await.map_err(map_storage_err)?;
        }

        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((true, updates))
    }

    pub async fn finance_budget_create(
        &self,
        params: FinanceBudgetCreateParams,
    ) -> HandlerResult<FinanceBudgetRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let start_date = params.start_date
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .unwrap_or_else(|| now.date_naive());
        let end_date = params.end_date
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());

        let row = FinanceBudgetRow {
            id: id.clone(),
            name: params.name,
            amount: params.amount,
            currency: params.currency.unwrap_or_else(|| "VND".to_string()),
            period: params.period,
            category: params.category,
            method: params.method.unwrap_or_else(|| "standard".to_string()),
            jar_type: None,
            start_date,
            end_date,
            is_active: true,
            alert_threshold: params.alert_threshold.unwrap_or(80),
            created_at: now,
            updated_at: now,
        };

        self.repos.finance.budgets.add(&row).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((row, updates))
    }

    pub async fn finance_budget_update(
        &self,
        params: FinanceBudgetUpdateParams,
    ) -> HandlerResult<FinanceBudgetRow> {
        let patch = FinanceBudgetPatch {
            id: params.id.clone(),
            name: params.name,
            amount: params.amount,
            category: params.category,
            is_active: params.is_active,
        };
        self.repos.finance.budgets.update(&patch).await.map_err(map_storage_err)?;
        let row = self.repos.finance.budgets.get(&params.id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id: params.id }];
        Ok((row, updates))
    }

    pub async fn finance_budget_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos.finance.budgets.delete(&id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((true, updates))
    }

    pub async fn finance_goal_create(
        &self,
        params: FinanceGoalCreateParams,
    ) -> HandlerResult<FinanceGoalRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let deadline = params.deadline
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());

        let row = FinanceGoalRow {
            id: id.clone(),
            name: params.name,
            goal_type: params.goal_type,
            target_amount: params.target_amount,
            current_amount: params.current_amount.unwrap_or(0),
            currency: params.currency.unwrap_or_else(|| "VND".to_string()),
            status: "active".to_string(),
            deadline,
            monthly_contribution: params.monthly_contribution,
            expected_return_rate: None,
            inflation_rate: None,
            notes: params.notes,
            created_at: now,
            updated_at: now,
        };

        self.repos.finance.goals.add(&row).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((row, updates))
    }

    pub async fn finance_goal_update(
        &self,
        params: FinanceGoalUpdateParams,
    ) -> HandlerResult<FinanceGoalRow> {
        let deadline = params.deadline.map(|opt| {
            opt.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        });
        let patch = FinanceGoalPatch {
            id: params.id.clone(),
            current_amount: params.current_amount,
            target_amount: params.target_amount,
            monthly_contribution: params.monthly_contribution,
            deadline,
            status: params.status,
            ..Default::default()
        };
        self.repos.finance.goals.update(&patch).await.map_err(map_storage_err)?;
        let row = self.repos.finance.goals.get(&params.id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id: params.id }];
        Ok((row, updates))
    }

    pub async fn finance_goal_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos.finance.goals.delete(&id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((true, updates))
    }

    pub async fn finance_liability_create(
        &self,
        params: FinanceLiabilityCreateParams,
    ) -> HandlerResult<FinanceLiabilityRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let due_date = params.due_date
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());

        let row = FinanceLiabilityRow {
            id: id.clone(),
            name: params.name,
            liability_type: params.liability_type,
            principal: params.principal,
            remaining: params.remaining.unwrap_or(params.principal),
            currency: params.currency.unwrap_or_else(|| "VND".to_string()),
            interest_rate: params.interest_rate,
            monthly_payment: params.monthly_payment,
            due_date,
            notes: params.notes,
            created_at: now,
            updated_at: now,
        };

        self.repos.finance.liabilities.add(&row).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((row, updates))
    }

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
        };
        self.repos.finance.liabilities.update(&patch).await.map_err(map_storage_err)?;
        let row = self.repos.finance.liabilities.get(&params.id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id: params.id }];
        Ok((row, updates))
    }

    pub async fn finance_liability_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos.finance.liabilities.delete(&id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((true, updates))
    }

    pub async fn finance_portfolio_create(
        &self,
        params: FinancePortfolioCreateParams,
    ) -> HandlerResult<FinancePortfolioRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let row = FinancePortfolioRow {
            id: id.clone(),
            name: params.name,
            description: params.description,
            currency: params.currency.unwrap_or_else(|| "VND".to_string()),
            created_at: now,
            updated_at: now,
        };
        self.repos.finance.investments.add_portfolio(&row).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((row, updates))
    }

    pub async fn finance_investment_create(
        &self,
        params: FinanceInvestmentCreateParams,
    ) -> HandlerResult<FinanceInvestmentRow> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let purchase_date = params.purchase_date
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());

        let row = FinanceInvestmentRow {
            id: id.clone(),
            portfolio_id: params.portfolio_id,
            asset_type: params.asset_type,
            symbol: params.symbol,
            name: params.name.unwrap_or_default(),
            quantity: params.quantity,
            cost_basis: params.cost_basis,
            currency: params.currency.unwrap_or_else(|| "VND".to_string()),
            current_price: None,
            current_value: None,
            purchase_date,
            notes: params.notes,
            created_at: now,
            updated_at: now,
        };
        self.repos.finance.investments.add_investment(&row).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id }];
        Ok((row, updates))
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
        self.repos.finance.investments.update_investment(&patch).await.map_err(map_storage_err)?;
        let row = self.repos.finance.investments.get_investment(&params.id).await.map_err(map_storage_err)?;
        let updates = vec![EntityUpdate { kind: EntityKind::Finance, id: params.id }];
        Ok((row, updates))
    }

    // ── Fixed existing queries ─────────────────────────────

    pub async fn finance_transactions_filtered(
        &self,
        params: FinanceTransactionFilterParams,
    ) -> Result<Vec<FinanceTransactionRow>, ApiError> {
        let filter = FinanceTransactionFilter {
            account_id: params.account_id,
            tx_type: params.tx_type,
            category: params.category,
            date_from: params.date_from.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            date_to: params.date_to.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            query: params.query,
            limit: params.limit,
            ..Default::default()
        };
        self.repos.finance.transactions.list(&filter).await.map_err(map_storage_err)
    }

    pub async fn finance_goals_all(&self, include_completed: bool) -> Result<Vec<FinanceGoalRow>, ApiError> {
        if include_completed {
            // list_active only returns active; for all goals we need a different query
            // For now, list active; full list requires a repo method addition
            self.repos.finance.goals.list_active().await.map_err(map_storage_err)
        } else {
            self.repos.finance.goals.list_active().await.map_err(map_storage_err)
        }
    }

    pub async fn finance_investments_filtered(
        &self,
        portfolio_id: Option<String>,
    ) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        let filter = FinanceInvestmentFilter {
            portfolio_id,
            ..Default::default()
        };
        self.repos.finance.investments.list_investments(&filter).await.map_err(map_storage_err)
    }

    // ── Reports ────────────────────────────────────────────

    pub async fn finance_report_spending(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<FinanceSpendingReportResponse, ApiError> {
        let from = date_from
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .unwrap_or_else(|| {
                let now = chrono::Utc::now().date_naive();
                now.with_day(1).unwrap_or(now)
            });
        let to = date_to
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let rows = self.repos.finance.transactions
            .sum_by_category(&from, &to, "expense")
            .await.map_err(map_storage_err)?;

        let total: i64 = rows.iter().map(|(_, amt)| amt).sum();
        let breakdown = rows.into_iter().map(|(category, amount)| {
            FinanceCategoryBreakdown {
                category,
                amount,
                pct: if total > 0 { (amount as f64 / total as f64) * 100.0 } else { 0.0 },
            }
        }).collect();

        Ok(FinanceSpendingReportResponse { total, breakdown })
    }

    pub async fn finance_report_income(
        &self,
        date_from: Option<String>,
        date_to: Option<String>,
    ) -> Result<FinanceSpendingReportResponse, ApiError> {
        let from = date_from
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .unwrap_or_else(|| {
                let now = chrono::Utc::now().date_naive();
                now.with_day(1).unwrap_or(now)
            });
        let to = date_to
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let rows = self.repos.finance.transactions
            .sum_by_category(&from, &to, "income")
            .await.map_err(map_storage_err)?;

        let total: i64 = rows.iter().map(|(_, amt)| amt).sum();
        let breakdown = rows.into_iter().map(|(category, amount)| {
            FinanceCategoryBreakdown {
                category,
                amount,
                pct: if total > 0 { (amount as f64 / total as f64) * 100.0 } else { 0.0 },
            }
        }).collect();

        Ok(FinanceSpendingReportResponse { total, breakdown })
    }

    pub async fn finance_report_trends(
        &self,
        metric: String,
        periods: Option<i64>,
    ) -> Result<Vec<FinanceTrendPoint>, ApiError> {
        let n = periods.unwrap_or(6).min(24);
        let tx_type = match metric.as_str() {
            "income" => "income",
            _ => "expense", // "spending" or "savings_rate"
        };
        let rows = self.repos.finance.transactions
            .sum_by_period(tx_type, n as usize, "monthly")
            .await.map_err(map_storage_err)?;

        let mut points: Vec<FinanceTrendPoint> = Vec::new();
        for (i, (period, value)) in rows.iter().enumerate() {
            let change_pct = if i > 0 {
                let prev = rows[i - 1].1;
                if prev > 0 { Some(((value - prev) as f64 / prev as f64) * 100.0) } else { None }
            } else {
                None
            };
            points.push(FinanceTrendPoint {
                period: period.clone(),
                value: *value,
                change_pct,
            });
        }

        Ok(points)
    }
```

Note: This requires adding the new imports at the top of the file. Add:
```rust
use desktop_shared::commands::{
    FinanceAccountCreateParams, FinanceAccountUpdateParams,
    FinanceTransactionCreateParams, FinanceTransactionUpdateParams, FinanceTransactionFilterParams,
    FinanceBudgetCreateParams, FinanceBudgetUpdateParams,
    FinanceGoalCreateParams, FinanceGoalUpdateParams,
    FinanceLiabilityCreateParams, FinanceLiabilityUpdateParams,
    FinancePortfolioCreateParams,
    FinanceInvestmentCreateParams, FinanceInvestmentUpdateParams,
    FinanceSpendingReportResponse, FinanceCategoryBreakdown, FinanceTrendPoint,
};
use desktop_shared::types::EntityKind;
use storage::rows::finance::{
    FinanceAccountPatch, FinanceTransactionPatch, FinanceBudgetPatch,
    FinanceGoalPatch, FinanceLiabilityPatch, FinanceInvestmentPatch, FinanceInvestmentFilter,
    FinancePortfolioRow,
};
use crate::state::{HandlerResult, EntityUpdate};
```

**Step 2: Verify it compiles**

Run: `cargo build -p app-core`

**Step 3: Commit**

```
feat(app-core): add finance mutation handlers and report queries
```

---

### Task 4: Add Tauri commands + dev server routes for mutations

**Files:**
- Modify: `crates/desktop/src/commands/finance.rs` (add mutation commands)
- Modify: `crates/desktop/src/main.rs:180-189` (register new commands)
- Modify: `crates/desktop/src/dev_server.rs:319-328` (add dev server routes)

**Step 1: Add Tauri mutation commands**

Append to `crates/desktop/src/commands/finance.rs`:

```rust
// Follow the pattern from notes.rs: (state, app, params) -> emit_updates

#[tauri::command]
pub async fn finance_account_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceAccountCreateParams,
) -> Result<FinanceAccountRow, ApiError> {
    let (result, updates) = state.finance_account_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ... (same pattern for all mutation commands)
```

Add all the Tauri commands following this exact pattern for:
- `finance_account_create`, `finance_account_update`, `finance_account_delete`
- `finance_transaction_create`, `finance_transaction_delete`
- `finance_budget_create`, `finance_budget_update`, `finance_budget_delete`
- `finance_goal_create`, `finance_goal_update`, `finance_goal_delete`
- `finance_liability_create`, `finance_liability_update`, `finance_liability_delete`
- `finance_portfolio_create`
- `finance_investment_create`, `finance_investment_update`

Also update the existing `finance_transactions` command to accept filter params, and add `finance_report_spending`, `finance_report_income`, `finance_report_trends`.

**Step 2: Register in main.rs invoke_handler**

Add all new command functions to the `invoke_handler` array in `crates/desktop/src/main.rs:180-189`.

**Step 3: Add dev server routes**

Add matching routes in `crates/desktop/src/dev_server.rs` after line 328.

**Step 4: Verify it compiles**

Run: `cargo build -p desktop`

**Step 5: Commit**

```
feat(desktop): add finance mutation Tauri commands and dev server routes
```

---

### Task 5: Fix exchange rates — store in config

**Files:**
- Modify: `crates/app-core/src/handlers/finance.rs` (replace the stub)

**Step 1: Replace the exchange rates stub**

Replace the `finance_exchange_rates` method with one that reads from the finance config section. The config already supports `finance.defaultCurrency` — add `finance.exchangeRates` as a `HashMap<String, f64>`:

```rust
pub async fn finance_exchange_rates(&self) -> Result<HashMap<String, f64>, ApiError> {
    // Read exchange rates from config finance section
    let config = self.config.read().await;
    Ok(config.finance.exchange_rates.clone().unwrap_or_default())
}
```

The config change requires adding `exchange_rates: Option<HashMap<String, f64>>` to the finance config struct in the `config` crate. This maps currency codes to their VND equivalent (e.g., `{"USD": 25500, "USDT": 25500}`).

**Step 2: Verify it compiles**

Run: `cargo build -p app-core`

**Step 3: Commit**

```
fix(app-core): implement exchange_rates from config instead of empty stub
```

---

## Phase 2: Frontend Shared Components

### Task 6: Update TypeScript types for new APIs

**Files:**
- Modify: `desktop-ui/src/lib/types.ts:402-498` (add mutation param types)

**Step 1: Add mutation and filter types**

Add after the existing `FinanceNetWorth` interface:

```typescript
// ── Mutation Params ─────────────────────────────────────────────
export interface FinanceAccountCreateParams {
  name: string;
  accountType: string;
  currency?: string;
  balance?: number;
  institution?: string;
  notes?: string;
}

export interface FinanceTransactionCreateParams {
  accountId: string;
  txType: "income" | "expense" | "transfer";
  amount: number;
  currency?: string;
  category?: string;
  subcategory?: string;
  counterparty?: string;
  txDate?: string;
  notes?: string;
}

export interface FinanceBudgetCreateParams {
  name: string;
  amount: number;
  period: string;
  currency?: string;
  category?: string;
  alertThreshold?: number;
}

export interface FinanceGoalCreateParams {
  name: string;
  goalType: string;
  targetAmount: number;
  currency?: string;
  currentAmount?: number;
  deadline?: string;
  monthlyContribution?: number;
  notes?: string;
}

export interface FinanceLiabilityCreateParams {
  name: string;
  liabilityType: string;
  principal: number;
  currency?: string;
  remaining?: number;
  interestRate?: number;
  monthlyPayment?: number;
  dueDate?: string;
  notes?: string;
}

export interface FinancePortfolioCreateParams {
  name: string;
  description?: string;
  currency?: string;
}

export interface FinanceInvestmentCreateParams {
  portfolioId: string;
  assetType: string;
  costBasis: number;
  quantity: number;
  symbol?: string;
  name?: string;
  currency?: string;
}

// ── Report Types ────────────────────────────────────────────────
export interface FinanceSpendingReport {
  total: number;
  breakdown: { category: string; amount: number; pct: number }[];
}

export interface FinanceTrendPoint {
  period: string;
  value: number;
  changePct: number | null;
}
```

**Step 2: Commit**

```
feat(desktop-ui): add finance mutation and report TypeScript types
```

---

### Task 7: Upgrade COLORS to CSS variable strings

**Files:**
- Modify: `desktop-ui/src/lib/finance.ts:84-93`

**Step 1: Replace hardcoded hex with CSS variable-based colors**

```typescript
// Semantic chart colors — CSS variables for theme adaptability,
// with hex fallbacks for SVG fill/stroke where CSS vars don't work.
export const CHART_COLORS = [
  { var: "var(--brand)", hex: "#f97316" },
  { var: "var(--info)", hex: "#3b82f6" },
  { var: "var(--success)", hex: "#22c55e" },
  { var: "var(--purple)", hex: "#8b5cf6" },
  { var: "var(--destructive)", hex: "#f43f5e" },
  { var: "var(--color-cyan-400)", hex: "#06b6d4" },
  { var: "var(--color-amber-500)", hex: "#f59e0b" },
  { var: "var(--color-pink-500)", hex: "#ec4899" },
];

// Keep COLORS for backward compat during migration
export const COLORS = CHART_COLORS.map((c) => c.hex);
```

**Step 2: Commit**

```
refactor(desktop-ui): upgrade COLORS to CSS variable-aware chart colors
```

---

### Task 8: Build AnimatedDonut component

**Files:**
- Modify: `desktop-ui/src/components/finance/Donut.tsx` (replace with animated version)

**Step 1: Rewrite the Donut with entrance animation**

Replace the entire file with an animated donut that:
- Uses `transition-[stroke-dashoffset] duration-700` on each segment
- Animates from 0 to full dash on mount via `useEffect` + state toggle
- Accepts optional `centerLabel` and `centerValue` as separate props
- Uses hex colors from `CHART_COLORS` for SVG strokes
- Adds `filter: drop-shadow()` glow matching the segment color
- Keeps the legend below

**Step 2: Commit**

```
feat(desktop-ui): animated donut chart with entrance transition and glow
```

---

### Task 9: Build FormModal component

**Files:**
- Create: `desktop-ui/src/components/finance/FormModal.tsx`

**Step 1: Create glass-panel modal**

Build a generic modal component:
- Backdrop: `fixed inset-0 bg-black/40 backdrop-blur-sm z-50`
- Container: `glass-panel rounded-2xl p-6 w-full max-w-md mx-auto mt-[15vh]`
- Staggered field entrance with `animation: fade-in 0.2s ease-out` + `animation-delay`
- Props: `open`, `onClose`, `title`, `children`, `onSubmit`
- Form fields use `glass-input` class
- Footer with Cancel (ghost) and Save (brand) buttons
- Escape key closes, click-outside closes

**Step 2: Build reusable FormField sub-component**

```tsx
function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-[11px] font-medium text-secondary">{label}</label>
      {children}
    </div>
  );
}
```

Input styling: `glass-input w-full px-3 py-2 text-[12px] font-light text-primary placeholder:text-dim rounded-lg`

**Step 3: Commit**

```
feat(desktop-ui): add FormModal glass-panel component for finance forms
```

---

### Task 10: Build SlidePanel component

**Files:**
- Create: `desktop-ui/src/components/finance/SlidePanel.tsx`

**Step 1: Create right-side drawer**

- Container: `fixed top-0 right-0 h-full w-[420px] glass-panel z-40`
- Slide-in animation: `translate-x-full → translate-x-0` with `transition-transform duration-300`
- Backdrop: `fixed inset-0 bg-black/20 z-30` (lighter than modal)
- Scrollable content area
- Header with title and close button
- Props: `open`, `onClose`, `title`, `children`

**Step 2: Commit**

```
feat(desktop-ui): add SlidePanel drawer component for transaction forms
```

---

### Task 11: Upgrade FinanceLayout with refresh button

**Files:**
- Modify: `desktop-ui/src/components/finance/FinanceLayout.tsx`

**Step 1: Wire the onRefresh prop to a button**

Add a refresh icon button in the tab bar that actually calls `onRefresh`:

```tsx
export function FinanceLayout({ children, onRefresh }: FinanceLayoutProps) {
  // ... existing code ...
  return (
    <div className="flex-1 flex flex-col gap-2 overflow-hidden">
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5" role="tablist">
          {/* existing tabs */}
        </div>
        {onRefresh && (
          <button
            type="button"
            onClick={onRefresh}
            className="ml-2 p-2 rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
          >
            <RefreshCw className="w-3.5 h-3.5" strokeWidth={1.5} />
          </button>
        )}
      </div>
      <div className="flex-1 overflow-y-auto p-4">{children}</div>
    </div>
  );
}
```

**Step 2: Commit**

```
fix(desktop-ui): wire FinanceLayout onRefresh prop to actual refresh button
```

---

### Task 12: Remove SectionLabel, upgrade Card component

**Files:**
- Modify: `desktop-ui/src/components/finance/Card.tsx`

**Step 1: Replace SectionLabel with internal card header**

Keep `Card` as-is (it's a thin wrapper, which is good), but remove `SectionLabel` export. Add a `CardHeader` export instead:

```tsx
export function CardHeader({
  title,
  subtitle,
  action,
}: {
  title: string;
  subtitle?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between mb-3">
      <h2 className="text-[13px] font-medium text-secondary">{title}</h2>
      <div className="flex items-center gap-2">
        {subtitle && <span className="text-[10px] font-light text-dim">{subtitle}</span>}
        {action}
      </div>
    </div>
  );
}
```

Keep `SectionLabel` exported but mark it deprecated — it will be removed once all pages are migrated.

**Step 2: Commit**

```
feat(desktop-ui): add CardHeader component matching productivity card pattern
```

---

## Phase 3: Page Rebuilds

### Task 13: Rebuild Finance Dashboard

**Files:**
- Modify: `desktop-ui/src/components/views/Finance.tsx` (full rewrite)

Key changes:
- Replace all `SectionLabel` with internal `CardHeader`
- Use `finance_report_spending` API for the spending donut instead of computing from 8 transactions
- `font-light` → `font-medium` on card titles
- `gap-3` → `gap-4`
- Net Worth card: `text-[28px] font-light leading-none` hero stat
- Fix account click-through: navigate to `/finance/accounts` and set `?id=` (FinanceAccounts will read it)
- Add loading skeletons using a simple `animate-pulse bg-white/[0.06] rounded` pattern
- Remove `<SectionLabel>&nbsp;</SectionLabel>` spacers

**Commit:** `feat(desktop-ui): rebuild Finance dashboard with upgraded visual design`

---

### Task 14: Rebuild FinanceAccounts with Add Account modal

**Files:**
- Modify: `desktop-ui/src/components/views/FinanceAccounts.tsx`

Key changes:
- Read `?id=` URL param via `useSearchParams` and auto-select the account
- Wire "Add Account" button to open `FormModal` with account fields
- Call `finance_account_create` on submit, then `refetchAll`
- Upgrade all `font-light` card titles to `font-medium`
- Replace `SectionLabel` with `CardHeader`
- `gap-3` → `gap-4`

**Commit:** `feat(desktop-ui): rebuild FinanceAccounts with Add Account modal and URL param support`

---

### Task 15: Rebuild FinanceTransactions with SlidePanel + server-side filtering

**Files:**
- Modify: `desktop-ui/src/components/views/FinanceTransactions.tsx`

Key changes:
- Replace client-side filtering with `finance_transactions` passing filter params
- Replace raw `<select>` and `<input>` with `glass-input` styled elements
- Wire "Add" button to open `SlidePanel` with transaction form
- Call `finance_transaction_create` on submit
- Debounce search input (300ms)
- Upgrade card titles, gap, colors

**Commit:** `feat(desktop-ui): rebuild FinanceTransactions with server-side filtering and Add Transaction panel`

---

### Task 16: Rebuild FinanceBudgets with modal + currency fix

**Files:**
- Modify: `desktop-ui/src/components/views/FinanceBudgets.tsx`

Key changes:
- Wire "Add Budget" button to `FormModal`
- Fetch `finance_exchange_rates` and apply conversion before summing totals
- Upgrade visual design (titles, gap, animated donut)

**Commit:** `feat(desktop-ui): rebuild FinanceBudgets with Add Budget modal and multi-currency fix`

---

### Task 17: Rebuild FinanceInvestments with Add Portfolio/Investment modals

**Files:**
- Modify: `desktop-ui/src/components/views/FinanceInvestments.tsx`

Key changes:
- Add "Add Portfolio" and "Add Investment" buttons + modals
- Call `finance_portfolio_create` / `finance_investment_create`
- Upgrade visual design

**Commit:** `feat(desktop-ui): rebuild FinanceInvestments with Add Portfolio/Investment modals`

---

### Task 18: Rebuild FinanceGoals with modal + status tabs

**Files:**
- Modify: `desktop-ui/src/components/views/FinanceGoals.tsx`

Key changes:
- Wire "Add Goal" button to `FormModal`
- Add status tab bar: Active / Achieved / Abandoned (fetch all goals, filter client-side)
  - Note: Backend currently only returns active. For now, keep Active-only; the tab bar is ready for when backend adds `list_all`
- Fix currency mixing: fetch exchange rates, convert before summing
- Replace static progress bars with animated ones (`transition-[width] duration-500`)

**Commit:** `feat(desktop-ui): rebuild FinanceGoals with Add Goal modal and status tabs`

---

### Task 19: Rebuild FinanceLiabilities with modal + Progress component

**Files:**
- Modify: `desktop-ui/src/components/views/FinanceLiabilities.tsx`

Key changes:
- Add "Add Liability" button + modal
- Replace raw `<div>` progress bars with `<Progress>` component
- Upgrade visual design

**Commit:** `feat(desktop-ui): rebuild FinanceLiabilities with Add Liability modal and Progress bars`

---

### Task 20: Add loading and error states to all finance pages

**Files:**
- Modify: All 7 finance view files

**Step 1: Create a shared loading skeleton**

Add to `desktop-ui/src/components/finance/FinanceSkeleton.tsx`:

```tsx
export function FinanceSkeleton({ rows = 4 }: { rows?: number }) {
  return (
    <div className="space-y-4 animate-pulse">
      <div className="grid grid-cols-4 gap-4">
        {[...Array(4)].map((_, i) => (
          <div key={i} className="glass-card p-4 h-20 rounded-xl" />
        ))}
      </div>
      {[...Array(rows)].map((_, i) => (
        <div key={i} className="glass-card p-4 h-16 rounded-xl" />
      ))}
    </div>
  );
}
```

**Step 2: Add loading/error to each page**

In each finance view, destructure `loading` and `error` from `useQuery` and render:
- Loading: `<FinanceSkeleton />`
- Error: `<Card className="p-6 text-center"><p className="text-destructive">...</p><button onClick={refetch}>Retry</button></Card>`

**Commit:** `feat(desktop-ui): add loading skeletons and error states to all finance pages`

---

### Task 21: Final lint and build verification

**Step 1: Run Biome**

```bash
cd desktop-ui && bun run lint:fix
```

**Step 2: Run Rust clippy**

```bash
cargo clippy --workspace --all-targets --all-features
```

**Step 3: Run frontend build**

```bash
cd desktop-ui && bun run build
```

**Step 4: Run Rust tests**

```bash
cargo nextest run --workspace
```

**Step 5: Commit any fixes**

```
chore: fix lint warnings and build errors from finance upgrade
```

---

## Summary

| Phase | Tasks | Key Deliverables |
|-------|-------|-----------------|
| 1: Backend | Tasks 1-5 | 18 mutation commands, fixed exchange rates, report queries, server-side tx filtering |
| 2: Components | Tasks 6-12 | AnimatedDonut, FormModal, SlidePanel, CardHeader, upgraded FinanceLayout, new TS types |
| 3: Pages | Tasks 13-21 | All 7 pages rebuilt with modern design, CRUD modals, loading/error states, lint-clean build |
