//! Plan-mode tagging helpers.
//!
//! When plan mode is active, all incoming items are forced to `Pending`
//! (validated separately) and the row is tagged with `proposed_in_plan_session`.
//! Ratification clears the tag without mutating items.

use bus::domain_events::TodoEvent;
use crate::types::TodoItemInput;

/// Generate a fresh plan session id (32-char hex, no dashes).
pub fn new_plan_session_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

/// Build a `TodoEvent::PlanProposed` for the items.
pub fn plan_proposed_event(
    thread_id: &str,
    plan_session_id: &str,
    items: &[TodoItemInput],
    timestamp: jiff::Timestamp,
) -> TodoEvent {
    TodoEvent::PlanProposed {
        thread_id: thread_id.into(),
        plan_session_id: plan_session_id.into(),
        item_ids: items.iter().filter_map(|i| i.id.clone()).collect(),
        timestamp,
    }
}

/// Build a `TodoEvent::PlanRatified`.
pub fn plan_ratified_event(
    thread_id: &str,
    plan_session_id: &str,
    ratified_count: usize,
    user_edited_count: usize,
    user_removed_count: usize,
    timestamp: jiff::Timestamp,
) -> TodoEvent {
    TodoEvent::PlanRatified {
        thread_id: thread_id.into(),
        plan_session_id: plan_session_id.into(),
        ratified_count,
        user_edited_count,
        user_removed_count,
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_plan_session_id_is_32_hex_chars() {
        let id = new_plan_session_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn new_plan_session_ids_are_unique() {
        let a = new_plan_session_id();
        let b = new_plan_session_id();
        assert_ne!(a, b);
    }

    #[test]
    fn plan_ratified_event_carries_counts() {
        let e = plan_ratified_event(
            "t1",
            "p_xyz",
            4,
            1,
            0,
            jiff::Timestamp::from_second(1_780_000_000).unwrap(),
        );
        match e {
            TodoEvent::PlanRatified {
                ratified_count,
                user_edited_count,
                user_removed_count,
                ..
            } => {
                assert_eq!(ratified_count, 4);
                assert_eq!(user_edited_count, 1);
                assert_eq!(user_removed_count, 0);
            }
            _ => panic!("wrong variant"),
        }
    }
}
