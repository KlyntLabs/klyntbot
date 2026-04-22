use ai_core::{AiEventMeta, AiSignal, RecallDomain, SignalConsumer, SignalRouter};
use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;

/// Translator: DomainEvent -> Option<AiSignal>. Returns None when the event
/// has no pipeline registration (e.g. transient infra events).
pub fn translate(event: &DomainEvent) -> Option<AiSignal> {
    // Tasks
    if let Some(e) = try_into_task_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Tasks;
        return Some(sig);
    }
    // Finance
    if let Some(e) = try_into_finance_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Finance;
        return Some(sig);
    }
    None
}

fn try_into_task_event(e: &DomainEvent) -> Option<feature_tasks::events::TaskEvent> {
    use feature_tasks::events::TaskEvent;
    match e {
        DomainEvent::TaskCreated {
            task_id,
            project,
            estimate_mins,
            ..
        } => Some(TaskEvent::Created {
            task_id: task_id.clone(),
            title: String::new(),   // title not in DomainEvent::TaskCreated
            area_id: String::new(), // area_id not in DomainEvent::TaskCreated
            project_id: project.clone(),
            priority: None,
            estimated_minutes: estimate_mins.map(|m| m as i32),
        }),
        DomainEvent::TaskCompleted {
            task_id,
            deviation_pct,
            ..
        } => Some(TaskEvent::Completed {
            task_id: task_id.clone(),
            title: String::new(), // title not in DomainEvent::TaskCompleted
            deviation_pct: *deviation_pct,
        }),
        DomainEvent::TaskFocusChanged {
            task_id,
            focus_deadline,
            ..
        } => Some(TaskEvent::FocusChanged {
            task_id: task_id.clone(),
            title: String::new(), // title not in DomainEvent::TaskFocusChanged
            focus_deadline: focus_deadline.as_ref().and_then(|d| d.parse().ok()),
        }),
        DomainEvent::EstimationRecorded {
            task_id,
            estimated_mins,
            actual_mins,
            ..
        } => Some(TaskEvent::EstimationRecorded {
            task_id: task_id.clone(),
            estimated_minutes: Some(*estimated_mins as i32),
            actual_minutes: Some(*actual_mins as i32),
            deviation_pct: 0.0, // calculated field
        }),
        _ => None,
    }
}

fn try_into_finance_event(e: &DomainEvent) -> Option<feature_finance::events::FinanceEvent> {
    use feature_finance::events::FinanceEvent;
    match e {
        DomainEvent::TransactionRecorded {
            category,
            amount,
            is_over_budget,
        } => Some(FinanceEvent::TransactionRecorded {
            _tx_id: String::new(), // not in the old variant
            category: category.clone(),
            amount: *amount as i64,
            currency: String::new(), // not in the old variant
            _is_over_budget: *is_over_budget,
        }),
        DomainEvent::BudgetAlert {
            category,
            spent,
            limit,
        } => Some(FinanceEvent::BudgetAlert {
            category: category.clone(),
            spent: *spent as i64,
            limit: *limit as i64,
        }),
        DomainEvent::AccountCreated {
            account_id,
            name,
            currency,
        } => Some(FinanceEvent::AccountCreated {
            account_id: account_id.clone(),
            name: name.clone(),
            currency: currency.clone(),
        }),
        DomainEvent::BudgetCreated {
            budget_id,
            name,
            amount,
            currency,
        } => Some(FinanceEvent::BudgetCreated {
            _budget_id: budget_id.clone(),
            name: name.clone(),
            amount: *amount as i64,
            currency: currency.clone(),
        }),
        DomainEvent::GoalCreated {
            goal_id,
            name,
            target_amount,
        } => Some(FinanceEvent::GoalCreated {
            goal_id: goal_id.clone(),
            name: name.clone(),
            target_amount: *target_amount as i64,
        }),
        DomainEvent::GoalAchieved { goal_id, name } => Some(FinanceEvent::GoalAchieved {
            goal_id: goal_id.clone(),
            name: name.clone(),
        }),
        _ => None,
    }
}

pub fn start(bus: Arc<DomainEventBus>, consumers: Vec<Arc<dyn SignalConsumer>>) -> SignalRouter {
    SignalRouter::start(bus, consumers, translate)
}
