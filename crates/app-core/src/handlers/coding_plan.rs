//! Plan-mode app-core handlers: enter, ratify, cancel, user-edit, user-remove,
//! plus helpers (compute_ratify_counts, plan-snapshot management,
//! untitled-rename watcher).

use feature_coding_todo::types::TodoItem;

/// Diff snapshot vs final to return (ratified, edited_or_added, removed) counts.
pub fn compute_ratify_counts(
    snapshot: Option<&[TodoItem]>,
    final_items: &[TodoItem],
) -> (usize, usize, usize) {
    use std::collections::HashMap;
    let snap = snapshot.unwrap_or(&[]);
    let snap_by_id: HashMap<&str, &TodoItem> = snap.iter().map(|i| (i.id.as_str(), i)).collect();
    let final_by_id: HashMap<&str, &TodoItem> = final_items.iter().map(|i| (i.id.as_str(), i)).collect();

    let removed = snap_by_id.keys().filter(|id| !final_by_id.contains_key(*id)).count();

    let mut ratified = 0usize;
    let mut edited = 0usize;
    for (id, fin) in &final_by_id {
        match snap_by_id.get(id) {
            Some(orig)
                if orig.title == fin.title
                    && orig.concurrency == fin.concurrency
                    && orig.blocked_by == fin.blocked_by =>
            {
                ratified += 1;
            }
            Some(_) | None => edited += 1,
        }
    }
    (ratified, edited, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::domain_events::{ConcurrencyClass, TodoStatus};
    use jiff::Timestamp;

    fn item(id: &str, title: &str) -> TodoItem {
        TodoItem {
            id: id.into(),
            title: title.into(),
            status: TodoStatus::Pending,
            concurrency: ConcurrencyClass::Sequential,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: Timestamp::from_second(1_780_000_000).unwrap(),
            updated_at: Timestamp::from_second(1_780_000_000).unwrap(),
        }
    }

    #[test]
    fn no_snapshot_means_all_edited() {
        let final_items = vec![item("a", "A"), item("b", "B")];
        assert_eq!(compute_ratify_counts(None, &final_items), (0, 2, 0));
    }

    #[test]
    fn unchanged_items_count_as_ratified() {
        let snap = vec![item("a", "A")];
        let final_items = vec![item("a", "A")];
        assert_eq!(compute_ratify_counts(Some(&snap), &final_items), (1, 0, 0));
    }

    #[test]
    fn modified_title_counts_as_edited() {
        let snap = vec![item("a", "A")];
        let final_items = vec![item("a", "A2")];
        assert_eq!(compute_ratify_counts(Some(&snap), &final_items), (0, 1, 0));
    }

    #[test]
    fn missing_in_final_counts_as_removed() {
        let snap = vec![item("a", "A"), item("b", "B")];
        let final_items = vec![item("a", "A")];
        assert_eq!(compute_ratify_counts(Some(&snap), &final_items), (1, 0, 1));
    }

    #[test]
    fn new_item_counts_as_edited() {
        let snap = vec![item("a", "A")];
        let final_items = vec![item("a", "A"), item("c", "C")];
        assert_eq!(compute_ratify_counts(Some(&snap), &final_items), (1, 1, 0));
    }
}
