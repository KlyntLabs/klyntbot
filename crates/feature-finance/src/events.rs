use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Finance")]
pub enum FinanceEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Transaction: {category} {amount} {currency}",
        coaching_signal(category_from = "category", amount_from = "amount",),
        metric(
            name = "budget_overrun_frequency",
            value_from = 0.0_f64,
            window = "30d",
            min_samples = 3,
            aggregation = "avg",
        )
    )]
    TransactionRecorded {
        _tx_id: String,
        category: String,
        amount: i64,
        currency: String,
        _is_over_budget: bool,
    },

    #[ai(
        importance = 0.9,
        salience = "extract",
        observation_template = "Budget alert: {category} spent {spent} of {limit}",
        entity_bridge(
            type = "finance_category",
            name_from = "category",
            id_from = "category"
        ),
        coaching_signal(
            category_from = "category",
            amount_from = "spent",
            rule = "Review spending patterns when budget pressure is detected",
        ),
        metric(
            name = "budget_overrun_frequency",
            value_from = 1.0_f64,
            window = "30d",
            min_samples = 3,
            aggregation = "avg",
        )
    )]
    BudgetAlert {
        category: String,
        spent: i64,
        limit: i64,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Account opened: {name} ({currency})",
        entity_bridge(type = "finance_account", name_from = "name", id_from = "account_id")
    )]
    AccountCreated {
        account_id: String,
        name: String,
        currency: String,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Budget created: {name} ({amount} {currency})"
    )]
    BudgetCreated {
        _budget_id: String,
        name: String,
        amount: i64,
        currency: String,
    },

    #[ai(
        importance = 0.6,
        salience = "accumulate",
        observation_template = "Goal created: {name} target {target_amount}",
        entity_bridge(type = "finance_goal", name_from = "name", id_from = "goal_id")
    )]
    GoalCreated {
        goal_id: String,
        name: String,
        target_amount: i64,
    },

    #[ai(
        importance = 0.9,
        salience = "extract",
        observation_template = "Goal achieved: {name}",
        entity_bridge(type = "finance_goal", name_from = "name", id_from = "goal_id")
    )]
    GoalAchieved { goal_id: String, name: String },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Goal '{name}' advanced by {delta} → {current_amount}/{target_amount}",
        entity_bridge(type = "finance_goal", name_from = "name", id_from = "goal_id"),
        metric(
            name = "goal_progress_velocity",
            value_from = *delta as f64,
            window = "30d",
            min_samples = 3,
            aggregation = "sum",
        ),
    )]
    GoalProgress {
        goal_id: String,
        name: String,
        current_amount: i64,
        target_amount: i64,
        delta: i64,
    },
}

impl From<FinanceEvent> for DomainEvent {
    fn from(e: FinanceEvent) -> Self {
        match e {
            FinanceEvent::TransactionRecorded {
                category,
                amount,
                _is_over_budget,
                ..
            } => DomainEvent::TransactionRecorded {
                category,
                amount: amount as f64,
                is_over_budget: _is_over_budget,
            },
            FinanceEvent::BudgetAlert {
                category,
                spent,
                limit,
            } => DomainEvent::BudgetAlert {
                category,
                spent: spent as f64,
                limit: limit as f64,
            },
            FinanceEvent::AccountCreated {
                account_id,
                name,
                currency,
            } => DomainEvent::AccountCreated {
                account_id,
                name,
                currency,
            },
            FinanceEvent::BudgetCreated {
                _budget_id,
                name,
                amount,
                currency,
            } => DomainEvent::BudgetCreated {
                budget_id: _budget_id,
                name,
                amount,
                currency,
            },
            FinanceEvent::GoalCreated {
                goal_id,
                name,
                target_amount,
            } => DomainEvent::GoalCreated {
                goal_id,
                name,
                target_amount,
            },
            FinanceEvent::GoalAchieved { goal_id, name } => {
                DomainEvent::GoalAchieved { goal_id, name }
            }
            FinanceEvent::GoalProgress {
                goal_id,
                name,
                current_amount,
                target_amount,
                delta,
            } => DomainEvent::FinanceGoalProgress {
                goal_id,
                name,
                current_amount,
                target_amount,
                delta,
            },
        }
    }
}

/// Translate a bus::DomainEvent into the typed FinanceEvent form, if it's a finance event.
pub fn try_from_domain_event(e: &DomainEvent) -> Option<FinanceEvent> {
    match e {
        DomainEvent::TransactionRecorded { category, amount, is_over_budget } => Some(FinanceEvent::TransactionRecorded {
            _tx_id: String::new(),
            category: category.clone(),
            amount: *amount as i64,
            currency: String::new(),
            _is_over_budget: *is_over_budget,
        }),
        DomainEvent::BudgetAlert { category, spent, limit } => Some(FinanceEvent::BudgetAlert {
            category: category.clone(),
            spent: *spent as i64,
            limit: *limit as i64,
        }),
        DomainEvent::AccountCreated { account_id, name, currency } => Some(FinanceEvent::AccountCreated {
            account_id: account_id.clone(),
            name: name.clone(),
            currency: currency.clone(),
        }),
        DomainEvent::BudgetCreated { budget_id, name, amount, currency } => Some(FinanceEvent::BudgetCreated {
            _budget_id: budget_id.clone(),
            name: name.clone(),
            amount: *amount,
            currency: currency.clone(),
        }),
        DomainEvent::GoalCreated { goal_id, name, target_amount } => Some(FinanceEvent::GoalCreated {
            goal_id: goal_id.clone(),
            name: name.clone(),
            target_amount: *target_amount,
        }),
        DomainEvent::GoalAchieved { goal_id, name } => Some(FinanceEvent::GoalAchieved {
            goal_id: goal_id.clone(),
            name: name.clone(),
        }),
        DomainEvent::FinanceGoalProgress { goal_id, name, current_amount, target_amount, delta } => Some(FinanceEvent::GoalProgress {
            goal_id: goal_id.clone(),
            name: name.clone(),
            current_amount: *current_amount,
            target_amount: *target_amount,
            delta: *delta,
        }),
        _ => None,
    }
}
