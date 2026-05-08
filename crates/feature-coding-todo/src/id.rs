//! ULID assignment for items missing `id`.
//!
//! ULIDs are time-ordered so creation order is preserved without an explicit
//! index column.

use crate::types::TodoItemInput;

/// Assign a ULID to every item without an `id`. Existing IDs are preserved.
pub fn assign_missing_ids(items: Vec<TodoItemInput>) -> Vec<TodoItemInput> {
    items
        .into_iter()
        .map(|mut i| {
            if i.id.is_none() {
                i.id = Some(ulid::Ulid::new().to_string());
            }
            i
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ConcurrencyClass, TodoStatus};

    fn item(id: Option<&str>) -> TodoItemInput {
        TodoItemInput {
            id: id.map(Into::into),
            title: "x".into(),
            status: TodoStatus::Pending,
            concurrency: ConcurrencyClass::Safe,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
        }
    }

    #[test]
    fn missing_id_gets_ulid() {
        let out = assign_missing_ids(vec![item(None)]);
        let assigned = out[0].id.as_ref().unwrap();
        assert_eq!(assigned.len(), 26, "ulid is 26 chars: {}", assigned);
    }

    #[test]
    fn existing_id_preserved() {
        let out = assign_missing_ids(vec![item(Some("preset"))]);
        assert_eq!(out[0].id.as_deref(), Some("preset"));
    }

    #[test]
    fn assigned_ids_are_unique() {
        let out = assign_missing_ids(vec![item(None), item(None), item(None)]);
        let ids: std::collections::HashSet<_> = out.iter().filter_map(|i| i.id.as_ref()).collect();
        assert_eq!(ids.len(), 3);
    }
}
