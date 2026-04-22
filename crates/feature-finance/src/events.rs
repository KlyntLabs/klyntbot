use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
pub enum FinanceEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Transaction: {category} {amount} {currency}"
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
        }
    }
}
