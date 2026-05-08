//! AiEvent wrapper for TodoEvent so it flows through the SignalRouter pipeline.

use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "General")]
pub enum CodingTodoEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Todo item {item_id} moved from {from:?} to {to:?}",
        metric(
            name = "todo_status_change_rate",
            value_from = 1.0_f64,
            window = "7d",
            min_samples = 5,
            aggregation = "sum"
        )
    )]
    StateChanged {
        thread_id: String,
        agent_id: String,
        agent_profile: String,
        item_id: String,
        from: bus::domain_events::TodoStatus,
        to: bus::domain_events::TodoStatus,
        concurrency: bus::domain_events::ConcurrencyClass,
        reason: Option<String>,
        timestamp: jiff::Timestamp,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Todo item {item_id} cancelled (was {prior_status:?})"
    )]
    Cancelled {
        thread_id: String,
        agent_id: String,
        agent_profile: String,
        item_id: String,
        prior_status: bus::domain_events::TodoStatus,
        was_blocked_by: Vec<String>,
        timestamp: jiff::Timestamp,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Plan proposed with {item_count} items"
    )]
    PlanProposed {
        thread_id: String,
        plan_session_id: String,
        item_count: usize,
        timestamp: jiff::Timestamp,
    },

    #[ai(
        importance = 0.6,
        salience = "accumulate",
        observation_template = "Plan ratified: {ratified_count} accepted, {user_edited_count} edited, {user_removed_count} removed"
    )]
    PlanRatified {
        thread_id: String,
        plan_session_id: String,
        ratified_count: usize,
        user_edited_count: usize,
        user_removed_count: usize,
        timestamp: jiff::Timestamp,
    },
}

/// Extract a `CodingTodoEvent` from a `DomainEvent::Todo` variant.
pub fn try_from_domain_event(e: &DomainEvent) -> Option<CodingTodoEvent> {
    match e {
        DomainEvent::Todo(bus::domain_events::TodoEvent::StateChanged {
            thread_id,
            agent_id,
            agent_profile,
            item_id,
            from,
            to,
            concurrency,
            reason,
            timestamp,
        }) => Some(CodingTodoEvent::StateChanged {
            thread_id: thread_id.clone(),
            agent_id: agent_id.clone(),
            agent_profile: agent_profile.clone(),
            item_id: item_id.clone(),
            from: *from,
            to: *to,
            concurrency: *concurrency,
            reason: reason.clone(),
            timestamp: *timestamp,
        }),
        DomainEvent::Todo(bus::domain_events::TodoEvent::Cancelled {
            thread_id,
            agent_id,
            agent_profile,
            item_id,
            prior_status,
            was_blocked_by,
            timestamp,
        }) => Some(CodingTodoEvent::Cancelled {
            thread_id: thread_id.clone(),
            agent_id: agent_id.clone(),
            agent_profile: agent_profile.clone(),
            item_id: item_id.clone(),
            prior_status: *prior_status,
            was_blocked_by: was_blocked_by.clone(),
            timestamp: *timestamp,
        }),
        DomainEvent::Todo(bus::domain_events::TodoEvent::PlanProposed {
            thread_id,
            plan_session_id,
            item_ids,
            timestamp,
        }) => Some(CodingTodoEvent::PlanProposed {
            thread_id: thread_id.clone(),
            plan_session_id: plan_session_id.clone(),
            item_count: item_ids.len(),
            timestamp: *timestamp,
        }),
        DomainEvent::Todo(bus::domain_events::TodoEvent::PlanRatified {
            thread_id,
            plan_session_id,
            ratified_count,
            user_edited_count,
            user_removed_count,
            timestamp,
        }) => Some(CodingTodoEvent::PlanRatified {
            thread_id: thread_id.clone(),
            plan_session_id: plan_session_id.clone(),
            ratified_count: *ratified_count,
            user_edited_count: *user_edited_count,
            user_removed_count: *user_removed_count,
            timestamp: *timestamp,
        }),
        _ => None,
    }
}
