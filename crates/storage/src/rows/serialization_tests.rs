//! Serialization tests for all storage row types.
//!
//! Verifies that every row type exposes `Serialize` + `#[serde(rename_all = "camelCase")]`
//! so the dashboard REST API can return camelCase JSON without any manual mapping.

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use uuid::Uuid;

    use crate::repos::project_repo::ProjectWithStats;
    use crate::repos::strategy::{OverallStats, ToolStatsRow};
    use crate::repos::todo_repo::TodoSummary;
    use crate::rows::calendar::{CalendarEventCacheRow, CalendarSyncStateRow};
    use crate::rows::cron::CronJobRow;
    use crate::rows::finance::{
        BudgetUsageRow, FinanceAccountRow, FinanceBudgetRow, FinanceGoalRow, FinanceInvestmentRow,
        FinanceInvestmentTxRow, FinanceLiabilityRow, FinancePortfolioRow, FinanceTransactionRow,
        PortfolioSummaryRow,
    };
    use crate::rows::goal::{GoalProjectLinkRow, GoalRow};
    use crate::rows::learning::{
        DecisionLogRow, EnrichmentFeedbackRow, LearningStateRow, OutcomeRow, StrategyRecordRow,
        StrategySummaryRow,
    };
    use crate::rows::memory::MemoryNoteRow;
    use crate::rows::plan::{PlanRow, PlanStepRow};
    use crate::rows::project::ProjectRow;
    use crate::rows::session::{SessionListRow, SessionMessageRow, SessionRow};
    use crate::rows::todo::{TodoAttachmentRow, TodoDependencyRow, TodoRow, TodoTimeEntryRow};
    use crate::rows::usage::UsageRecordRow;

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Assert that every key in a `serde_json::Value::Object` is camelCase.
    /// Fails the test if any key contains an underscore (snake_case).
    fn assert_no_snake_case_keys(value: &serde_json::Value, context: &str) {
        if let serde_json::Value::Object(map) = value {
            for key in map.keys() {
                assert!(
                    !key.contains('_'),
                    "snake_case key `{}` found in {} — expected camelCase",
                    key,
                    context
                );
                assert_no_snake_case_keys(&map[key], context);
            }
        }
    }

    fn sample_todo_row() -> TodoRow {
        TodoRow {
            id: "task-1".to_string(),
            title: "Test task".to_string(),
            description: None,
            priority: Some(2),
            due_date: None,
            tags: vec![],
            status: "todo".to_string(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            parent_id: None,
            project_id: None,
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
        }
    }

    fn sample_project_row() -> ProjectRow {
        ProjectRow {
            id: "proj-1".to_string(),
            name: "Test project".to_string(),
            description: None,
            color: "#ff0000".to_string(),
            tags: vec![],
            status: "active".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ── TodoRow ──────────────────────────────────────────────────────────────

    #[test]
    fn todo_row_camel_case() {
        // AC-1.1: TodoRow serializes all fields to camelCase keys
        let row = sample_todo_row();
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "TodoRow");
        // Spot-check specific camelCase keys exist
        assert!(v.get("dueDate").is_some());
        assert!(v.get("focusedAt").is_some());
        assert!(v.get("focusDeadline").is_some());
        assert!(v.get("focusExpiredCount").is_some());
        assert!(v.get("createdAt").is_some());
        assert!(v.get("updatedAt").is_some());
        assert!(v.get("completedAt").is_some());
        assert!(v.get("parentId").is_some());
        assert!(v.get("projectId").is_some());
        assert!(v.get("totalTrackedSecs").is_some());
        assert!(v.get("estimatedMinutes").is_some());
        assert!(v.get("calendarEventUid").is_some());
        assert!(v.get("lastRemindedAt").is_some());
        assert!(v.get("recurrenceRule").is_some());
        assert!(v.get("recurrenceParentId").is_some());
        assert!(v.get("isTemplate").is_some());
        assert!(v.get("nextInstanceDate").is_some());
    }

    #[test]
    fn todo_row_round_trip() {
        // AC-1.1: TodoRow serializes → deserializes back to identical struct
        let row = sample_todo_row();
        let json = serde_json::to_string(&row).unwrap();
        // Verify it's valid JSON with no snake_case keys
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_no_snake_case_keys(&v, "TodoRow round-trip");
    }

    #[test]
    fn todo_attachment_row_camel_case() {
        // AC-1.2: TodoAttachmentRow serializes to camelCase
        let row = TodoAttachmentRow {
            id: Uuid::new_v4(),
            todo_id: "task-1".to_string(),
            attachment_type: "url".to_string(),
            value: "https://example.com".to_string(),
            title: None,
            tags: vec![],
            created_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "TodoAttachmentRow");
        assert!(v.get("todoId").is_some());
        assert!(v.get("attachmentType").is_some());
        assert!(v.get("createdAt").is_some());
    }

    #[test]
    fn todo_time_entry_row_camel_case() {
        // AC-1.2: TodoTimeEntryRow serializes to camelCase
        let row = TodoTimeEntryRow {
            id: Uuid::new_v4(),
            todo_id: "task-1".to_string(),
            source: "manual".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            duration_secs: None,
            note: None,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "TodoTimeEntryRow");
        assert!(v.get("todoId").is_some());
        assert!(v.get("startedAt").is_some());
        assert!(v.get("endedAt").is_some());
        assert!(v.get("durationSecs").is_some());
    }

    #[test]
    fn todo_dependency_row_camel_case() {
        // AC-1.2: TodoDependencyRow serializes to camelCase
        let row = TodoDependencyRow {
            task_id: "task-1".to_string(),
            blocker_id: "task-2".to_string(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "TodoDependencyRow");
        assert!(v.get("taskId").is_some());
        assert!(v.get("blockerId").is_some());
    }

    // ── ProjectRow ────────────────────────────────────────────────────────────

    #[test]
    fn project_row_camel_case() {
        // AC-1.3: ProjectRow serializes all fields to camelCase
        let row = sample_project_row();
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "ProjectRow");
        assert!(v.get("createdAt").is_some());
        assert!(v.get("updatedAt").is_some());
    }

    #[test]
    fn project_row_round_trip() {
        // AC-1.3: ProjectRow round-trip serialization
        let row = sample_project_row();
        let json = serde_json::to_string(&row).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_no_snake_case_keys(&v, "ProjectRow round-trip");
    }

    // ── PlanRow / PlanStepRow ─────────────────────────────────────────────────

    #[test]
    fn plan_row_camel_case() {
        // AC-1.4: PlanRow serializes to camelCase
        let row = PlanRow {
            id: Uuid::new_v4(),
            session_key: "session-1".to_string(),
            goal_id: None,
            title: "Test plan".to_string(),
            description: "Description".to_string(),
            status: "draft".to_string(),
            current_step_index: 0,
            iteration_limit: 10,
            backtrack_history: serde_json::Value::Array(vec![]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "PlanRow");
        assert!(v.get("sessionKey").is_some());
        assert!(v.get("goalId").is_some());
        assert!(v.get("currentStepIndex").is_some());
        assert!(v.get("iterationLimit").is_some());
        assert!(v.get("backtrackHistory").is_some());
        assert!(v.get("createdAt").is_some());
        assert!(v.get("updatedAt").is_some());
        assert!(v.get("completedAt").is_some());
    }

    #[test]
    fn plan_step_row_camel_case() {
        // AC-1.4: PlanStepRow serializes to camelCase
        let row = PlanStepRow {
            id: Uuid::new_v4(),
            plan_id: Uuid::new_v4(),
            step_index: 0,
            description: "Step 1".to_string(),
            reasoning: "Because".to_string(),
            expected_tools: vec!["todo_add".to_string()],
            status: "pending".to_string(),
            attempt_count: 0,
            max_attempts: 3,
            result: None,
            started_at: None,
            completed_at: None,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "PlanStepRow");
        assert!(v.get("planId").is_some());
        assert!(v.get("stepIndex").is_some());
        assert!(v.get("expectedTools").is_some());
        assert!(v.get("attemptCount").is_some());
        assert!(v.get("maxAttempts").is_some());
        assert!(v.get("startedAt").is_some());
        assert!(v.get("completedAt").is_some());
    }

    // ── SessionRow / SessionMessageRow / SessionListRow ────────────────────────

    #[test]
    fn session_row_camel_case() {
        // AC-1.5: SessionRow serializes to camelCase
        let row = SessionRow {
            key: "session-1".to_string(),
            metadata: serde_json::Value::Object(Default::default()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "SessionRow");
        assert!(v.get("createdAt").is_some());
        assert!(v.get("updatedAt").is_some());
    }

    #[test]
    fn session_message_row_camel_case() {
        // AC-1.5: SessionMessageRow serializes to camelCase
        let row = SessionMessageRow {
            id: Uuid::new_v4(),
            session_key: "session-1".to_string(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: Utc::now(),
            request_id: None,
            tool_calls: None,
            metadata: None,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "SessionMessageRow");
        assert!(v.get("sessionKey").is_some());
        assert!(v.get("requestId").is_some());
        assert!(v.get("toolCalls").is_some());
    }

    #[test]
    fn session_list_row_camel_case() {
        // AC-1.5: SessionListRow serializes to camelCase
        let row = SessionListRow {
            key: "session-1".to_string(),
            metadata: serde_json::Value::Object(Default::default()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            message_count: 5,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "SessionListRow");
        assert!(v.get("createdAt").is_some());
        assert!(v.get("updatedAt").is_some());
        assert!(v.get("messageCount").is_some());
    }

    // ── CalendarRow types ─────────────────────────────────────────────────────

    #[test]
    fn calendar_sync_state_row_camel_case() {
        // AC-1.6: CalendarSyncStateRow serializes to camelCase
        let row = CalendarSyncStateRow {
            provider_id: "google".to_string(),
            sync_token: None,
            last_sync_at: None,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "CalendarSyncStateRow");
        assert!(v.get("providerId").is_some());
        assert!(v.get("syncToken").is_some());
        assert!(v.get("lastSyncAt").is_some());
    }

    #[test]
    fn calendar_event_cache_row_camel_case() {
        // AC-1.6: CalendarEventCacheRow serializes to camelCase
        let row = CalendarEventCacheRow {
            uid: "event-1".to_string(),
            provider_id: "google".to_string(),
            summary: "Meeting".to_string(),
            description: None,
            start_at: Utc::now(),
            end_at: Utc::now(),
            source: "caldav".to_string(),
            etag: None,
            status: None,
            cached_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "CalendarEventCacheRow");
        assert!(v.get("providerId").is_some());
        assert!(v.get("startAt").is_some());
        assert!(v.get("endAt").is_some());
        assert!(v.get("cachedAt").is_some());
    }

    // ── CronJobRow ────────────────────────────────────────────────────────────

    #[test]
    fn cron_job_row_camel_case() {
        // AC-1.7: CronJobRow serializes to camelCase
        let row = CronJobRow {
            id: "cron-1".to_string(),
            name: "daily".to_string(),
            enabled: true,
            schedule: serde_json::json!({"type": "cron", "expr": "0 9 * * *"}),
            payload: serde_json::json!({}),
            next_run_at_ms: None,
            last_run_at_ms: None,
            last_status: None,
            last_error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
            delete_after_run: false,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "CronJobRow");
        assert!(v.get("nextRunAtMs").is_some());
        assert!(v.get("lastRunAtMs").is_some());
        assert!(v.get("deleteAfterRun").is_some());
    }

    // ── UsageRecordRow ────────────────────────────────────────────────────────

    #[test]
    fn usage_record_row_camel_case() {
        // AC-1.x: UsageRecordRow serializes to camelCase
        let row = UsageRecordRow {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id: "req-1".to_string(),
            model: "claude-sonnet".to_string(),
            provider: "anthropic".to_string(),
            prompt_tokens: 100,
            completion_tokens: 50,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated_cost_usd: 0.001,
            channel: "cli".to_string(),
            strategy: "direct".to_string(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "UsageRecordRow");
        assert!(v.get("requestId").is_some());
        assert!(v.get("promptTokens").is_some());
        assert!(v.get("completionTokens").is_some());
        assert!(v.get("cacheReadTokens").is_some());
        assert!(v.get("cacheWriteTokens").is_some());
        assert!(v.get("estimatedCostUsd").is_some());
    }

    // ── Finance row types (10 types) ──────────────────────────────────────────

    #[test]
    fn finance_account_row_camel_case() {
        // AC-1.8: FinanceAccountRow serializes to camelCase
        let row = FinanceAccountRow {
            id: "acc-1".to_string(),
            name: "Checking".to_string(),
            account_type: "checking".to_string(),
            currency: "USD".to_string(),
            balance: 10000,
            institution: None,
            notes: None,
            is_archived: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinanceAccountRow");
        assert!(v.get("accountType").is_some());
        assert!(v.get("isArchived").is_some());
        assert!(v.get("createdAt").is_some());
        assert!(v.get("updatedAt").is_some());
    }

    #[test]
    fn finance_transaction_row_camel_case() {
        // AC-1.8: FinanceTransactionRow serializes to camelCase
        let row = FinanceTransactionRow {
            id: "tx-1".to_string(),
            account_id: "acc-1".to_string(),
            tx_type: "expense".to_string(),
            amount: 500,
            currency: "USD".to_string(),
            category: None,
            subcategory: None,
            counterparty: None,
            notes: None,
            tx_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            transfer_id: None,
            is_recurring: false,
            recurring_rule: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinanceTransactionRow");
        assert!(v.get("accountId").is_some());
        assert!(v.get("txType").is_some());
        assert!(v.get("txDate").is_some());
        assert!(v.get("isRecurring").is_some());
        assert!(v.get("recurringRule").is_some());
        assert!(v.get("createdAt").is_some());
    }

    #[test]
    fn finance_budget_row_camel_case() {
        // AC-1.8: FinanceBudgetRow serializes to camelCase
        let row = FinanceBudgetRow {
            id: "budget-1".to_string(),
            name: "Groceries".to_string(),
            amount: 50000,
            currency: "USD".to_string(),
            period: "monthly".to_string(),
            category: Some("food".to_string()),
            method: "50-30-20".to_string(),
            jar_type: None,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end_date: None,
            is_active: true,
            alert_threshold: 80,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinanceBudgetRow");
        assert!(v.get("jarType").is_some());
        assert!(v.get("startDate").is_some());
        assert!(v.get("endDate").is_some());
        assert!(v.get("isActive").is_some());
        assert!(v.get("alertThreshold").is_some());
    }

    #[test]
    fn finance_portfolio_row_camel_case() {
        // AC-1.8: FinancePortfolioRow serializes to camelCase
        let row = FinancePortfolioRow {
            id: "port-1".to_string(),
            name: "Growth".to_string(),
            description: None,
            currency: "USD".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinancePortfolioRow");
        assert!(v.get("createdAt").is_some());
        assert!(v.get("updatedAt").is_some());
    }

    #[test]
    fn finance_investment_row_camel_case() {
        // AC-1.8: FinanceInvestmentRow serializes to camelCase
        let row = FinanceInvestmentRow {
            id: "inv-1".to_string(),
            portfolio_id: "port-1".to_string(),
            asset_type: "stock".to_string(),
            symbol: Some("AAPL".to_string()),
            name: "Apple Inc".to_string(),
            quantity: 10.0,
            cost_basis: 150000,
            currency: "USD".to_string(),
            current_price: None,
            current_value: None,
            purchase_date: None,
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinanceInvestmentRow");
        assert!(v.get("portfolioId").is_some());
        assert!(v.get("assetType").is_some());
        assert!(v.get("costBasis").is_some());
        assert!(v.get("currentPrice").is_some());
        assert!(v.get("currentValue").is_some());
        assert!(v.get("purchaseDate").is_some());
    }

    #[test]
    fn finance_investment_tx_row_camel_case() {
        // AC-1.8: FinanceInvestmentTxRow serializes to camelCase
        let row = FinanceInvestmentTxRow {
            id: "invtx-1".to_string(),
            investment_id: "inv-1".to_string(),
            tx_type: "buy".to_string(),
            quantity: Some(10.0),
            price_per_unit: Some(15000),
            total_amount: 150000,
            currency: "USD".to_string(),
            fees: 0,
            tx_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            notes: None,
            created_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinanceInvestmentTxRow");
        assert!(v.get("investmentId").is_some());
        assert!(v.get("txType").is_some());
        assert!(v.get("pricePerUnit").is_some());
        assert!(v.get("totalAmount").is_some());
        assert!(v.get("txDate").is_some());
    }

    #[test]
    fn finance_goal_row_camel_case() {
        // AC-1.8: FinanceGoalRow serializes to camelCase
        let row = FinanceGoalRow {
            id: "fgoal-1".to_string(),
            name: "Emergency fund".to_string(),
            goal_type: "savings".to_string(),
            target_amount: 1000000,
            current_amount: 200000,
            currency: "USD".to_string(),
            status: "active".to_string(),
            deadline: None,
            monthly_contribution: None,
            expected_return_rate: None,
            inflation_rate: None,
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinanceGoalRow");
        assert!(v.get("goalType").is_some());
        assert!(v.get("targetAmount").is_some());
        assert!(v.get("currentAmount").is_some());
        assert!(v.get("monthlyContribution").is_some());
        assert!(v.get("expectedReturnRate").is_some());
        assert!(v.get("inflationRate").is_some());
    }

    #[test]
    fn finance_liability_row_camel_case() {
        // AC-1.8: FinanceLiabilityRow serializes to camelCase
        let row = FinanceLiabilityRow {
            id: "liab-1".to_string(),
            name: "Car loan".to_string(),
            liability_type: "loan".to_string(),
            principal: 2000000,
            remaining: 1500000,
            currency: "USD".to_string(),
            interest_rate: Some(5.0),
            monthly_payment: Some(30000),
            due_date: None,
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "FinanceLiabilityRow");
        assert!(v.get("liabilityType").is_some());
        assert!(v.get("interestRate").is_some());
        assert!(v.get("monthlyPayment").is_some());
        assert!(v.get("dueDate").is_some());
    }

    #[test]
    fn budget_usage_row_camel_case() {
        // AC-1.8: BudgetUsageRow serializes to camelCase
        let row = BudgetUsageRow {
            id: "budget-1".to_string(),
            name: "Groceries".to_string(),
            amount: 50000,
            currency: "USD".to_string(),
            period: "monthly".to_string(),
            category: Some("food".to_string()),
            method: "50-30-20".to_string(),
            jar_type: None,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end_date: None,
            is_active: true,
            alert_threshold: 80,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            spent: 30000,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "BudgetUsageRow");
        assert!(v.get("jarType").is_some());
        assert!(v.get("startDate").is_some());
        assert!(v.get("isActive").is_some());
        assert!(v.get("alertThreshold").is_some());
    }

    #[test]
    fn portfolio_summary_row_camel_case() {
        // AC-1.8: PortfolioSummaryRow serializes to camelCase
        let row = PortfolioSummaryRow {
            portfolio_id: "port-1".to_string(),
            total_cost_basis: 150000,
            total_current_value: 180000,
            holding_count: 1,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert_no_snake_case_keys(&v, "PortfolioSummaryRow");
        assert!(v.get("portfolioId").is_some());
        assert!(v.get("totalCostBasis").is_some());
        assert!(v.get("totalCurrentValue").is_some());
        assert!(v.get("holdingCount").is_some());
    }

    // ── Aggregate types ────────────────────────────────────────────────────────

    #[test]
    fn todo_summary_camel_case() {
        // AC-1.9: TodoSummary aggregate type serializes to camelCase
        let summary = TodoSummary {
            todo: 5,
            doing: 2,
            done: 10,
            total: 17,
        };
        let v = serde_json::to_value(&summary).unwrap();
        assert_no_snake_case_keys(&v, "TodoSummary");
        // All fields are single words — just verify it serializes correctly
        assert!(v.get("todo").is_some());
        assert!(v.get("doing").is_some());
        assert!(v.get("done").is_some());
        assert!(v.get("total").is_some());
    }

    #[test]
    fn project_with_stats_camel_case() {
        // AC-1.9: ProjectWithStats serializes to camelCase (all nested fields included)
        let stats = ProjectWithStats {
            project: sample_project_row(),
            task_count_todo: 3,
            task_count_doing: 1,
            task_count_done: 5,
            task_count_total: 9,
        };
        let v = serde_json::to_value(&stats).unwrap();
        assert_no_snake_case_keys(&v, "ProjectWithStats");
        assert!(v.get("taskCountTodo").is_some());
        assert!(v.get("taskCountDoing").is_some());
        assert!(v.get("taskCountDone").is_some());
        assert!(v.get("taskCountTotal").is_some());
        // Nested ProjectRow fields should also be camelCase
        let project = v.get("project").unwrap();
        assert_no_snake_case_keys(project, "ProjectWithStats.project");
    }

    // ── Cross-cutting check ────────────────────────────────────────────────────

    #[test]
    fn no_snake_case_keys_in_any_serialized_row() {
        // AC-1.10: Verify no snake_case key appears in any serialized row type.
        let now = Utc::now();
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

        // TodoRow
        assert_no_snake_case_keys(&serde_json::to_value(sample_todo_row()).unwrap(), "TodoRow");

        // TodoAttachmentRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&TodoAttachmentRow {
                id: Uuid::new_v4(),
                todo_id: "t".to_string(),
                attachment_type: "url".to_string(),
                value: "v".to_string(),
                title: None,
                tags: vec![],
                created_at: now,
            })
            .unwrap(),
            "TodoAttachmentRow",
        );

        // TodoTimeEntryRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&TodoTimeEntryRow {
                id: Uuid::new_v4(),
                todo_id: "t".to_string(),
                source: "manual".to_string(),
                started_at: now,
                ended_at: None,
                duration_secs: None,
                note: None,
            })
            .unwrap(),
            "TodoTimeEntryRow",
        );

        // TodoDependencyRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&TodoDependencyRow {
                task_id: "t1".to_string(),
                blocker_id: "t2".to_string(),
            })
            .unwrap(),
            "TodoDependencyRow",
        );

        // ProjectRow
        assert_no_snake_case_keys(
            &serde_json::to_value(sample_project_row()).unwrap(),
            "ProjectRow",
        );

        // PlanRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&PlanRow {
                id: Uuid::new_v4(),
                session_key: "s".to_string(),
                goal_id: None,
                title: "t".to_string(),
                description: "d".to_string(),
                status: "draft".to_string(),
                current_step_index: 0,
                iteration_limit: 5,
                backtrack_history: serde_json::json!([]),
                created_at: now,
                updated_at: now,
                completed_at: None,
            })
            .unwrap(),
            "PlanRow",
        );

        // PlanStepRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&PlanStepRow {
                id: Uuid::new_v4(),
                plan_id: Uuid::new_v4(),
                step_index: 0,
                description: "d".to_string(),
                reasoning: "r".to_string(),
                expected_tools: vec![],
                status: "pending".to_string(),
                attempt_count: 0,
                max_attempts: 3,
                result: None,
                started_at: None,
                completed_at: None,
            })
            .unwrap(),
            "PlanStepRow",
        );

        // SessionRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&SessionRow {
                key: "s".to_string(),
                metadata: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "SessionRow",
        );

        // SessionMessageRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&SessionMessageRow {
                id: Uuid::new_v4(),
                session_key: "s".to_string(),
                role: "user".to_string(),
                content: "hi".to_string(),
                timestamp: now,
                request_id: None,
                tool_calls: None,
                metadata: None,
            })
            .unwrap(),
            "SessionMessageRow",
        );

        // SessionListRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&SessionListRow {
                key: "s".to_string(),
                metadata: serde_json::json!({}),
                created_at: now,
                updated_at: now,
                message_count: 0,
            })
            .unwrap(),
            "SessionListRow",
        );

        // CalendarSyncStateRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&CalendarSyncStateRow {
                provider_id: "g".to_string(),
                sync_token: None,
                last_sync_at: None,
            })
            .unwrap(),
            "CalendarSyncStateRow",
        );

        // CalendarEventCacheRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&CalendarEventCacheRow {
                uid: "u".to_string(),
                provider_id: "g".to_string(),
                summary: "s".to_string(),
                description: None,
                start_at: now,
                end_at: now,
                source: "caldav".to_string(),
                etag: None,
                status: None,
                cached_at: now,
            })
            .unwrap(),
            "CalendarEventCacheRow",
        );

        // CronJobRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&CronJobRow {
                id: "c".to_string(),
                name: "n".to_string(),
                enabled: true,
                schedule: serde_json::json!({}),
                payload: serde_json::json!({}),
                next_run_at_ms: None,
                last_run_at_ms: None,
                last_status: None,
                last_error: None,
                created_at_ms: 0,
                updated_at_ms: 0,
                delete_after_run: false,
            })
            .unwrap(),
            "CronJobRow",
        );

        // UsageRecordRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&UsageRecordRow {
                id: Uuid::new_v4(),
                timestamp: now,
                request_id: "r".to_string(),
                model: "m".to_string(),
                provider: "p".to_string(),
                prompt_tokens: 0,
                completion_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                estimated_cost_usd: 0.0,
                channel: "cli".to_string(),
                strategy: "direct".to_string(),
            })
            .unwrap(),
            "UsageRecordRow",
        );

        // FinanceAccountRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinanceAccountRow {
                id: "a".to_string(),
                name: "n".to_string(),
                account_type: "checking".to_string(),
                currency: "USD".to_string(),
                balance: 0,
                institution: None,
                notes: None,
                is_archived: false,
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "FinanceAccountRow",
        );

        // FinanceTransactionRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinanceTransactionRow {
                id: "t".to_string(),
                account_id: "a".to_string(),
                tx_type: "expense".to_string(),
                amount: 0,
                currency: "USD".to_string(),
                category: None,
                subcategory: None,
                counterparty: None,
                notes: None,
                tx_date: today,
                transfer_id: None,
                is_recurring: false,
                recurring_rule: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "FinanceTransactionRow",
        );

        // FinanceBudgetRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinanceBudgetRow {
                id: "b".to_string(),
                name: "n".to_string(),
                amount: 0,
                currency: "USD".to_string(),
                period: "monthly".to_string(),
                category: None,
                method: "m".to_string(),
                jar_type: None,
                start_date: today,
                end_date: None,
                is_active: true,
                alert_threshold: 80,
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "FinanceBudgetRow",
        );

        // FinancePortfolioRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinancePortfolioRow {
                id: "p".to_string(),
                name: "n".to_string(),
                description: None,
                currency: "USD".to_string(),
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "FinancePortfolioRow",
        );

        // FinanceInvestmentRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinanceInvestmentRow {
                id: "i".to_string(),
                portfolio_id: "p".to_string(),
                asset_type: "stock".to_string(),
                symbol: None,
                name: "n".to_string(),
                quantity: 0.0,
                cost_basis: 0,
                currency: "USD".to_string(),
                current_price: None,
                current_value: None,
                purchase_date: None,
                notes: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "FinanceInvestmentRow",
        );

        // FinanceInvestmentTxRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinanceInvestmentTxRow {
                id: "t".to_string(),
                investment_id: "i".to_string(),
                tx_type: "buy".to_string(),
                quantity: None,
                price_per_unit: None,
                total_amount: 0,
                currency: "USD".to_string(),
                fees: 0,
                tx_date: today,
                notes: None,
                created_at: now,
            })
            .unwrap(),
            "FinanceInvestmentTxRow",
        );

        // FinanceGoalRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinanceGoalRow {
                id: "g".to_string(),
                name: "n".to_string(),
                goal_type: "savings".to_string(),
                target_amount: 0,
                current_amount: 0,
                currency: "USD".to_string(),
                status: "active".to_string(),
                deadline: None,
                monthly_contribution: None,
                expected_return_rate: None,
                inflation_rate: None,
                notes: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "FinanceGoalRow",
        );

        // FinanceLiabilityRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&FinanceLiabilityRow {
                id: "l".to_string(),
                name: "n".to_string(),
                liability_type: "loan".to_string(),
                principal: 0,
                remaining: 0,
                currency: "USD".to_string(),
                interest_rate: None,
                monthly_payment: None,
                due_date: None,
                notes: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "FinanceLiabilityRow",
        );

        // BudgetUsageRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&BudgetUsageRow {
                id: "b".to_string(),
                name: "n".to_string(),
                amount: 0,
                currency: "USD".to_string(),
                period: "monthly".to_string(),
                category: None,
                method: "m".to_string(),
                jar_type: None,
                start_date: today,
                end_date: None,
                is_active: true,
                alert_threshold: 80,
                created_at: now,
                updated_at: now,
                spent: 0,
            })
            .unwrap(),
            "BudgetUsageRow",
        );

        // PortfolioSummaryRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&PortfolioSummaryRow {
                portfolio_id: "p".to_string(),
                total_cost_basis: 0,
                total_current_value: 0,
                holding_count: 0,
            })
            .unwrap(),
            "PortfolioSummaryRow",
        );

        // GoalRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&GoalRow {
                id: Uuid::new_v4(),
                title: "t".to_string(),
                description: "d".to_string(),
                status: "active".to_string(),
                priority: 2,
                target_date: None,
                created_at: now,
                updated_at: now,
                plans_completed: 0,
                plans_failed: 0,
                avg_duration_ms: None,
                last_plan_at: None,
            })
            .unwrap(),
            "GoalRow",
        );

        // GoalProjectLinkRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&GoalProjectLinkRow {
                goal_id: Uuid::new_v4(),
                project_id: "p".to_string(),
            })
            .unwrap(),
            "GoalProjectLinkRow",
        );

        // OutcomeRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&OutcomeRow {
                id: "o".to_string(),
                session_key: "s".to_string(),
                tool_name: "todo_add".to_string(),
                success: true,
                error_category: None,
                duration_ms: 100,
                confidence_score: None,
                confidence_dimensions: None,
                execution_mode: serde_json::json!("direct"),
                created_at: now,
            })
            .unwrap(),
            "OutcomeRow",
        );

        // StrategyRecordRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&StrategyRecordRow {
                id: Uuid::new_v4(),
                timestamp: now,
                request_id: "r".to_string(),
                predicted_strategy: "direct".to_string(),
                actual_strategy: "direct".to_string(),
                escalation_count: 0,
                iterations_used: 1,
                max_iterations: 5,
                success: true,
                user_satisfaction: None,
                response_time_ms: 500,
                chat_id: None,
                tool_name: None,
                tool_success: None,
                tool_duration_ms: None,
            })
            .unwrap(),
            "StrategyRecordRow",
        );

        // EnrichmentFeedbackRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&EnrichmentFeedbackRow {
                id: 1,
                task_id: "t".to_string(),
                field: "priority".to_string(),
                suggested_value: "1".to_string(),
                actual_value: None,
                accepted: true,
                confidence: 0.9,
                timestamp: now,
            })
            .unwrap(),
            "EnrichmentFeedbackRow",
        );

        // LearningStateRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&LearningStateRow {
                key: "k".to_string(),
                value: serde_json::json!({}),
                updated_at: now,
            })
            .unwrap(),
            "LearningStateRow",
        );

        // StrategySummaryRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&StrategySummaryRow {
                predicted_strategy: "direct".to_string(),
                sample_count: 10,
                correct_count: 9,
                avg_escalations: 0.1,
            })
            .unwrap(),
            "StrategySummaryRow",
        );

        // DecisionLogRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&DecisionLogRow {
                id: "d".to_string(),
                session_key: "s".to_string(),
                iteration: 1,
                tool_names: serde_json::json!([]),
                user_message_preview: "hi".to_string(),
                assessment: serde_json::json!({}),
                outcome: None,
                created_at: now,
            })
            .unwrap(),
            "DecisionLogRow",
        );

        // MemoryNoteRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&MemoryNoteRow {
                note_key: "2026-02-24".to_string(),
                content: "notes".to_string(),
                created_at: now,
                updated_at: now,
            })
            .unwrap(),
            "MemoryNoteRow",
        );

        // TodoSummary
        assert_no_snake_case_keys(
            &serde_json::to_value(&TodoSummary {
                todo: 5,
                doing: 2,
                done: 10,
                total: 17,
            })
            .unwrap(),
            "TodoSummary",
        );

        // ProjectWithStats
        assert_no_snake_case_keys(
            &serde_json::to_value(&ProjectWithStats {
                project: sample_project_row(),
                task_count_todo: 0,
                task_count_doing: 0,
                task_count_done: 0,
                task_count_total: 0,
            })
            .unwrap(),
            "ProjectWithStats",
        );

        // OverallStats
        assert_no_snake_case_keys(
            &serde_json::to_value(&OverallStats {
                total_records: 0,
                accuracy: 1.0,
                avg_response_time_ms: 0,
                avg_satisfaction: None,
            })
            .unwrap(),
            "OverallStats",
        );

        // ToolStatsRow
        assert_no_snake_case_keys(
            &serde_json::to_value(&ToolStatsRow {
                tool_name: "todo_add".to_string(),
                total_calls: 0,
                success_count: 0,
                avg_duration_ms: 0,
            })
            .unwrap(),
            "ToolStatsRow",
        );
    }
}
