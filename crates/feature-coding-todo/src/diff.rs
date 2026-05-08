//! Diff prior vs new item lists to produce events.

use bus::domain_events::TodoEvent;
use crate::types::{TodoItemInput, TodoStatus};

#[derive(Debug, Clone, PartialEq)]
pub struct DiffSummary {
    pub added: Vec<String>,
    pub status_changed: Vec<StatusChange>,
    pub cancelled: Vec<CancelledItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatusChange {
    pub item_id: String,
    pub from: TodoStatus,
    pub to: TodoStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CancelledItem {
    pub item_id: String,
    pub prior_status: TodoStatus,
    pub was_blocked_by: Vec<String>,
}

impl DiffSummary {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.status_changed.is_empty() && self.cancelled.is_empty()
    }
}

pub fn compute_diff(prior: &[TodoItemInput], new: &[TodoItemInput]) -> DiffSummary {
    use std::collections::HashMap;

    let prior_map: HashMap<&str, &TodoItemInput> = prior
        .iter()
        .filter_map(|i| i.id.as_deref().map(|id| (id, i)))
        .collect();
    let new_map: HashMap<&str, &TodoItemInput> = new
        .iter()
        .filter_map(|i| i.id.as_deref().map(|id| (id, i)))
        .collect();

    let mut added: Vec<String> = Vec::new();
    let mut status_changed: Vec<StatusChange> = Vec::new();

    for (id, item) in &new_map {
        match prior_map.get(id) {
            None => added.push(id.to_string()),
            Some(prev) if prev.status != item.status => status_changed.push(StatusChange {
                item_id: id.to_string(),
                from: prev.status,
                to: item.status,
            }),
            _ => {}
        }
    }

    let mut cancelled: Vec<CancelledItem> = Vec::new();
    for (id, item) in &prior_map {
        if !new_map.contains_key(id) {
            cancelled.push(CancelledItem {
                item_id: id.to_string(),
                prior_status: item.status,
                was_blocked_by: item.blocked_by.clone(),
            });
        }
    }

    DiffSummary {
        added,
        status_changed,
        cancelled,
    }
}

/// Metadata repeated in every TodoEvent variant.
pub struct EventMeta {
    pub thread_id: String,
    pub agent_id: String,
    pub agent_profile: String,
    pub timestamp: jiff::Timestamp,
}

/// Convert a DiffSummary + the new list into a Vec<TodoEvent>.
pub fn diff_to_events(
    diff: &DiffSummary,
    new_items: &[TodoItemInput],
    meta: &EventMeta,
) -> Vec<TodoEvent> {
    use std::collections::HashMap;

    let new_by_id: HashMap<&str, &TodoItemInput> = new_items
        .iter()
        .filter_map(|i| i.id.as_deref().map(|id| (id, i)))
        .collect();

    let mut out: Vec<TodoEvent> = Vec::new();

    // Status changes — emit StateChanged with from/to.
    for sc in &diff.status_changed {
        let item = match new_by_id.get(sc.item_id.as_str()) {
            Some(i) => i,
            None => continue,
        };
        out.push(TodoEvent::StateChanged {
            thread_id: meta.thread_id.clone(),
            agent_id: meta.agent_id.clone(),
            agent_profile: meta.agent_profile.clone(),
            item_id: sc.item_id.clone(),
            from: sc.from,
            to: sc.to,
            concurrency: item.concurrency,
            reason: item.blocked_reason.clone(),
            timestamp: meta.timestamp,
        });
    }

    // Added items — emit StateChanged from Pending->Pending (no-op transition,
    // but useful for the cognitive layer's "this item entered the system" signal).
    for id in &diff.added {
        if let Some(item) = new_by_id.get(id.as_str()) {
            out.push(TodoEvent::StateChanged {
                thread_id: meta.thread_id.clone(),
                agent_id: meta.agent_id.clone(),
                agent_profile: meta.agent_profile.clone(),
                item_id: id.clone(),
                from: item.status,
                to: item.status,
                concurrency: item.concurrency,
                reason: item.blocked_reason.clone(),
                timestamp: meta.timestamp,
            });
        }
    }

    // Cancelled items.
    for c in &diff.cancelled {
        out.push(TodoEvent::Cancelled {
            thread_id: meta.thread_id.clone(),
            agent_id: meta.agent_id.clone(),
            agent_profile: meta.agent_profile.clone(),
            item_id: c.item_id.clone(),
            prior_status: c.prior_status,
            was_blocked_by: c.was_blocked_by.clone(),
            timestamp: meta.timestamp,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ConcurrencyClass;

    fn it(id: &str, status: TodoStatus) -> TodoItemInput {
        TodoItemInput {
            id: Some(id.into()),
            title: format!("title for {}", id),
            status,
            concurrency: ConcurrencyClass::Safe,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
        }
    }

    #[test]
    fn empty_prior_all_added() {
        let new = vec![it("a", TodoStatus::Pending), it("b", TodoStatus::Pending)];
        let d = compute_diff(&[], &new);
        assert_eq!(d.added.len(), 2);
        assert!(d.status_changed.is_empty());
        assert!(d.cancelled.is_empty());
    }

    #[test]
    fn status_change_detected() {
        let prior = vec![it("a", TodoStatus::Pending)];
        let new = vec![it("a", TodoStatus::InProgress)];
        let d = compute_diff(&prior, &new);
        assert_eq!(d.status_changed.len(), 1);
        assert_eq!(d.status_changed[0].from, TodoStatus::Pending);
        assert_eq!(d.status_changed[0].to, TodoStatus::InProgress);
        assert!(d.added.is_empty());
        assert!(d.cancelled.is_empty());
    }

    #[test]
    fn dropped_item_is_cancelled() {
        let prior = vec![it("a", TodoStatus::Pending), it("b", TodoStatus::InProgress)];
        let new = vec![it("a", TodoStatus::Pending)];
        let d = compute_diff(&prior, &new);
        assert_eq!(d.cancelled.len(), 1);
        assert_eq!(d.cancelled[0].item_id, "b");
        assert_eq!(d.cancelled[0].prior_status, TodoStatus::InProgress);
    }

    #[test]
    fn cancelled_carries_blocked_by() {
        let mut a = it("a", TodoStatus::Pending);
        a.blocked_by = vec!["dep1".into(), "dep2".into()];
        let prior = vec![a];
        let d = compute_diff(&prior, &[]);
        assert_eq!(d.cancelled[0].was_blocked_by, vec!["dep1", "dep2"]);
    }

    #[test]
    fn diff_to_events_emits_state_changed() {
        let prior = vec![it("a", TodoStatus::Pending)];
        let new = vec![it("a", TodoStatus::InProgress)];
        let diff = compute_diff(&prior, &new);
        let meta = EventMeta {
            thread_id: "t1".into(),
            agent_id: "root".into(),
            agent_profile: "root".into(),
            timestamp: jiff::Timestamp::from_second(1_780_000_000).unwrap(),
        };
        let evts = diff_to_events(&diff, &new, &meta);
        assert_eq!(evts.len(), 1);
        match &evts[0] {
            TodoEvent::StateChanged { from, to, .. } => {
                assert_eq!(*from, TodoStatus::Pending);
                assert_eq!(*to, TodoStatus::InProgress);
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn diff_to_events_emits_cancelled() {
        let prior = vec![it("a", TodoStatus::InProgress)];
        let new: Vec<TodoItemInput> = vec![];
        let diff = compute_diff(&prior, &new);
        let meta = EventMeta {
            thread_id: "t1".into(),
            agent_id: "root".into(),
            agent_profile: "root".into(),
            timestamp: jiff::Timestamp::from_second(1_780_000_000).unwrap(),
        };
        let evts = diff_to_events(&diff, &new, &meta);
        assert_eq!(evts.len(), 1);
        match &evts[0] {
            TodoEvent::Cancelled { item_id, prior_status, .. } => {
                assert_eq!(item_id, "a");
                assert_eq!(*prior_status, TodoStatus::InProgress);
            }
            other => panic!("unexpected: {:?}", other),
        }
    }
}
