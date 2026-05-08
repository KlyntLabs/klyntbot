//! Pure validation helpers operating on `Vec<TodoItemInput>`.
//!
//! Each fn returns `Result<(), CodingTodoError>` — composed by `validate_write`
//! in a fixed order so error reports name the offending item even when multiple
//! invariants are violated.

use crate::errors::CodingTodoError;
use crate::types::{ConcurrencyClass, TodoItemInput, TodoStatus};

fn item_id(i: &TodoItemInput) -> String {
    i.id.clone().unwrap_or_else(|| i.title.clone())
}

/// Reject if more than one item in the list has status=InProgress.
pub fn validate_in_progress_per_agent(
    agent_id: &str,
    items: &[TodoItemInput],
) -> Result<(), CodingTodoError> {
    let mut in_progress = items.iter().filter(|i| i.status == TodoStatus::InProgress);
    let _ = in_progress.next();
    if in_progress.next().is_some() {
        let ids: Vec<String> = items
            .iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .map(item_id)
            .collect();
        return Err(CodingTodoError::MultipleInProgressInAgent {
            agent_id: agent_id.into(),
            item_ids: ids,
        });
    }
    Ok(())
}

/// Reject if any item with status=Blocked is missing a non-empty `blocked_reason`.
pub fn validate_blocked_has_reason(items: &[TodoItemInput]) -> Result<(), CodingTodoError> {
    for i in items {
        if i.status == TodoStatus::Blocked
            && i.blocked_reason.as_deref().map(str::trim).unwrap_or("").is_empty()
        {
            return Err(CodingTodoError::BlockedItemMissingReason {
                item_id: item_id(i),
            });
        }
    }
    Ok(())
}

/// Reject if any item references a `blocked_by` id that doesn't exist in the same list.
pub fn validate_blocked_by_known_items(items: &[TodoItemInput]) -> Result<(), CodingTodoError> {
    let known: std::collections::HashSet<&str> =
        items.iter().filter_map(|i| i.id.as_deref()).collect();
    for i in items {
        for dep in &i.blocked_by {
            if !known.contains(dep.as_str()) {
                return Err(CodingTodoError::BlockedByUnknownItem {
                    item_id: item_id(i),
                    missing_dep: dep.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Reject if the `blocked_by` graph contains a cycle.
pub fn validate_blocked_by_no_cycle(items: &[TodoItemInput]) -> Result<(), CodingTodoError> {
    use std::collections::HashMap;

    // Build adjacency: id -> list of deps
    let graph: HashMap<&str, Vec<&str>> = items
        .iter()
        .filter_map(|i| i.id.as_deref().map(|id| (id, i.blocked_by.iter().map(String::as_str).collect())))
        .collect();

    // 0=unvisited, 1=in-stack, 2=done
    let mut state: HashMap<&str, u8> = graph.keys().map(|k| (*k, 0u8)).collect();
    let mut path: Vec<&str> = Vec::new();

    fn visit<'a>(
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        state: &mut HashMap<&'a str, u8>,
        path: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        match state.get(node).copied().unwrap_or(0) {
            1 => {
                // Cycle detected — return path slice from the recurrence
                let cycle_start = path.iter().position(|p| *p == node).unwrap_or(0);
                let mut cycle: Vec<String> = path[cycle_start..].iter().map(|s| s.to_string()).collect();
                cycle.push(node.to_string());
                return Some(cycle);
            }
            2 => return None,
            _ => {}
        }
        state.insert(node, 1);
        path.push(node);
        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if let Some(cycle) = visit(dep, graph, state, path) {
                    return Some(cycle);
                }
            }
        }
        path.pop();
        state.insert(node, 2);
        None
    }

    for &node in graph.keys() {
        if let Some(cycle) = visit(node, &graph, &mut state, &mut path) {
            return Err(CodingTodoError::CycleInBlockedBy { chain: cycle });
        }
    }
    Ok(())
}

/// For any item whose `blocked_by` references items not yet `Done`, coerce
/// status to Blocked with synthetic `blocked_reason`. Returns mutated copy.
pub fn auto_coerce_blocked_for_unmet_deps(items: Vec<TodoItemInput>) -> Vec<TodoItemInput> {
    let done: std::collections::HashSet<String> = items
        .iter()
        .filter(|i| i.status == TodoStatus::Done)
        .filter_map(|i| i.id.clone())
        .collect();

    items
        .into_iter()
        .map(|mut i| {
            let mut unmet_iter = i.blocked_by.iter().filter(|d| !done.contains(d.as_str()));
            if let Some(first) = unmet_iter.next() {
                if i.status != TodoStatus::Blocked && i.status != TodoStatus::Done {
                    i.status = TodoStatus::Blocked;
                    if i.blocked_reason.is_none() {
                        let mut reason = format!("waiting on {}", first);
                        for dep in unmet_iter {
                            reason.push_str(", ");
                            reason.push_str(dep);
                        }
                        i.blocked_reason = Some(reason);
                    }
                }
            }
            i
        })
        .collect()
}

/// If the writing agent's profile is `explore` (read-only), force every item's
/// concurrency class to `Safe` regardless of what the LLM declared.
pub fn apply_explore_profile_safe_default(
    profile: &str,
    items: Vec<TodoItemInput>,
) -> Vec<TodoItemInput> {
    if profile != "explore" {
        return items;
    }
    items
        .into_iter()
        .map(|mut i| {
            i.concurrency = ConcurrencyClass::Safe;
            i
        })
        .collect()
}

/// Cross-agent invariant. Given the items being written for `caller_agent` and
/// the existing in_progress items from sibling agents (parameter
/// `other_agents_in_progress`: Vec<(agent_id, item_id, class)>), reject if a
/// transition to InProgress would conflict.
///
/// Rules:
///   - Exclusive: rejects if ANY other agent has any InProgress item.
///   - Sequential: rejects if any other agent has Sequential or Exclusive InProgress.
///   - Safe: never rejected.
pub fn validate_concurrency_cross_agent(
    items: &[TodoItemInput],
    other_agents_in_progress: &[(String, String, ConcurrencyClass)],
) -> Result<(), CodingTodoError> {
    for i in items {
        if i.status != TodoStatus::InProgress {
            continue;
        }
        let conflicts: Vec<(String, String)> = other_agents_in_progress
            .iter()
            .filter(|(_, _, other_class)| match (i.concurrency, *other_class) {
                // Another agent with Exclusive blocks ALL InProgress here.
                (ConcurrencyClass::Safe, ConcurrencyClass::Exclusive) => true,
                (ConcurrencyClass::Safe, _) => false,
                (ConcurrencyClass::Sequential, ConcurrencyClass::Safe) => false,
                (ConcurrencyClass::Sequential, _) => true,
                (ConcurrencyClass::Exclusive, _) => true,
            })
            .map(|(a, id, _)| (a.clone(), id.clone()))
            .collect();
        if !conflicts.is_empty() {
            return Err(CodingTodoError::ConcurrencyViolation {
                item_id: item_id(i),
                class: i.concurrency,
                conflicts_with: conflicts,
            });
        }
    }
    Ok(())
}

/// In plan mode every item must have status=Pending.
pub fn validate_plan_mode_pending_only(
    plan_mode_active: bool,
    items: &[TodoItemInput],
) -> Result<(), CodingTodoError> {
    if !plan_mode_active {
        return Ok(());
    }
    for i in items {
        if i.status != TodoStatus::Pending {
            return Err(CodingTodoError::PlanModeNonPendingStatus {
                item_id: item_id(i),
                status: i.status,
            });
        }
    }
    Ok(())
}

/// Anti-passivity: if the previous coding_todo call already had blocked items
/// without a paired user-facing message, reject this call when the same
/// condition is true. Caller passes `previous_violation` (true on consecutive
/// turn) and `same_turn_user_msg_emitted` (whether a user-facing assistant
/// message has been emitted in the current iteration).
pub fn validate_anti_passivity(
    items: &[TodoItemInput],
    ctx: &ValidationContext<'_>,
) -> Result<(), CodingTodoError> {
    if ctx.same_turn_user_msg_emitted || !ctx.previous_anti_passivity_violation {
        return Ok(());
    }
    let blocked_ids: Vec<String> = items
        .iter()
        .filter(|i| i.status == TodoStatus::Blocked)
        .map(item_id)
        .collect();
    if blocked_ids.is_empty() {
        return Ok(());
    }
    Err(CodingTodoError::BlockedItemMissingUserMessage {
        item_ids: blocked_ids,
    })
}

/// Cross-agent state used by composed validation. Caller supplies snapshot.
pub struct ValidationContext<'a> {
    pub agent_id: &'a str,
    pub agent_profile: &'a str,
    pub plan_mode_active: bool,
    pub previous_anti_passivity_violation: bool,
    pub same_turn_user_msg_emitted: bool,
    pub other_agents_in_progress: &'a [(String, String, ConcurrencyClass)],
}

/// Run all validators in a fixed order. Returns the (possibly mutated) items
/// after `auto_coerce_blocked_for_unmet_deps` and
/// `apply_explore_profile_safe_default`.
pub fn validate_write(
    items: Vec<TodoItemInput>,
    ctx: &ValidationContext<'_>,
) -> Result<Vec<TodoItemInput>, CodingTodoError> {
    // 1. Profile auto-classification (mutates concurrency).
    let items = apply_explore_profile_safe_default(ctx.agent_profile, items);

    // 2. Auto-coerce status for unmet deps (mutates status).
    let items = auto_coerce_blocked_for_unmet_deps(items);

    // 3. Pure validators.
    validate_in_progress_per_agent(ctx.agent_id, &items)?;
    validate_blocked_has_reason(&items)?;
    validate_blocked_by_known_items(&items)?;
    validate_blocked_by_no_cycle(&items)?;
    validate_plan_mode_pending_only(ctx.plan_mode_active, &items)?;
    validate_concurrency_cross_agent(&items, ctx.other_agents_in_progress)?;
    validate_anti_passivity(&items, ctx)?;
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, status: TodoStatus) -> TodoItemInput {
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
    fn in_progress_per_agent_allows_zero() {
        let items = vec![item("a", TodoStatus::Pending), item("b", TodoStatus::Done)];
        assert!(validate_in_progress_per_agent("root", &items).is_ok());
    }

    #[test]
    fn in_progress_per_agent_allows_one() {
        let items = vec![item("a", TodoStatus::Pending), item("b", TodoStatus::InProgress)];
        assert!(validate_in_progress_per_agent("root", &items).is_ok());
    }

    #[test]
    fn in_progress_per_agent_rejects_two() {
        let items = vec![
            item("a", TodoStatus::InProgress),
            item("b", TodoStatus::InProgress),
        ];
        let err = validate_in_progress_per_agent("root", &items).unwrap_err();
        match err {
            CodingTodoError::MultipleInProgressInAgent { agent_id, item_ids } => {
                assert_eq!(agent_id, "root");
                assert_eq!(item_ids, vec!["a", "b"]);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    fn blocked_with_reason(id: &str, reason: &str) -> TodoItemInput {
        TodoItemInput {
            id: Some(id.into()),
            title: format!("title for {}", id),
            status: TodoStatus::Blocked,
            concurrency: ConcurrencyClass::Safe,
            blocked_reason: Some(reason.into()),
            blocked_by: vec![],
            delegated_to: None,
        }
    }

    #[test]
    fn blocked_must_have_reason() {
        let mut bad = item("a", TodoStatus::Blocked);
        bad.blocked_reason = None;
        let r = validate_blocked_has_reason(&[bad]);
        assert!(matches!(r, Err(CodingTodoError::BlockedItemMissingReason { .. })));
    }

    #[test]
    fn blocked_empty_reason_rejected() {
        let mut bad = item("a", TodoStatus::Blocked);
        bad.blocked_reason = Some("   ".into());
        let r = validate_blocked_has_reason(&[bad]);
        assert!(matches!(r, Err(CodingTodoError::BlockedItemMissingReason { .. })));
    }

    #[test]
    fn blocked_with_reason_passes() {
        assert!(validate_blocked_has_reason(&[blocked_with_reason("a", "waiting on x")]).is_ok());
    }

    #[test]
    fn non_blocked_doesnt_need_reason() {
        let i = item("a", TodoStatus::Pending);
        assert!(validate_blocked_has_reason(&[i]).is_ok());
    }

    #[test]
    fn blocked_by_existing_id_passes() {
        let a = item("a", TodoStatus::Pending);
        let mut b = item("b", TodoStatus::Pending);
        b.blocked_by = vec!["a".into()];
        assert!(validate_blocked_by_known_items(&[a, b]).is_ok());
    }

    #[test]
    fn blocked_by_unknown_id_rejected() {
        let mut a = item("a", TodoStatus::Pending);
        a.blocked_by = vec!["ghost".into()];
        let err = validate_blocked_by_known_items(&[a]).unwrap_err();
        match err {
            CodingTodoError::BlockedByUnknownItem { missing_dep, .. } => {
                assert_eq!(missing_dep, "ghost");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn linear_chain_passes() {
        // a -> b -> c (a depends on b which depends on c)
        let mut a = item("a", TodoStatus::Pending);
        a.blocked_by = vec!["b".into()];
        let mut b = item("b", TodoStatus::Pending);
        b.blocked_by = vec!["c".into()];
        let c = item("c", TodoStatus::Pending);
        assert!(validate_blocked_by_no_cycle(&[a, b, c]).is_ok());
    }

    #[test]
    fn self_cycle_rejected() {
        let mut a = item("a", TodoStatus::Pending);
        a.blocked_by = vec!["a".into()];
        let r = validate_blocked_by_no_cycle(&[a]);
        assert!(matches!(r, Err(CodingTodoError::CycleInBlockedBy { .. })));
    }

    #[test]
    fn two_node_cycle_rejected() {
        let mut a = item("a", TodoStatus::Pending);
        a.blocked_by = vec!["b".into()];
        let mut b = item("b", TodoStatus::Pending);
        b.blocked_by = vec!["a".into()];
        let r = validate_blocked_by_no_cycle(&[a, b]);
        assert!(matches!(r, Err(CodingTodoError::CycleInBlockedBy { .. })));
    }

    #[test]
    fn coerce_pending_with_unmet_dep_to_blocked() {
        let a = item("a", TodoStatus::Pending); // dep not done
        let mut b = item("b", TodoStatus::Pending);
        b.blocked_by = vec!["a".into()];
        let out = auto_coerce_blocked_for_unmet_deps(vec![a, b]);
        assert_eq!(out[1].status, TodoStatus::Blocked);
        assert_eq!(out[1].blocked_reason.as_deref(), Some("waiting on a"));
    }

    #[test]
    fn dont_coerce_when_dep_done() {
        let a = item("a", TodoStatus::Done);
        let mut b = item("b", TodoStatus::Pending);
        b.blocked_by = vec!["a".into()];
        let out = auto_coerce_blocked_for_unmet_deps(vec![a, b]);
        assert_eq!(out[1].status, TodoStatus::Pending);
    }

    #[test]
    fn dont_overwrite_existing_blocked_reason() {
        let a = item("a", TodoStatus::Pending);
        let mut b = item("b", TodoStatus::Blocked);
        b.blocked_by = vec!["a".into()];
        b.blocked_reason = Some("user clarification".into());
        let out = auto_coerce_blocked_for_unmet_deps(vec![a, b]);
        assert_eq!(out[1].blocked_reason.as_deref(), Some("user clarification"));
    }

    #[test]
    fn explore_profile_forces_safe() {
        let mut a = item("a", TodoStatus::Pending);
        a.concurrency = ConcurrencyClass::Exclusive;
        let out = apply_explore_profile_safe_default("explore", vec![a]);
        assert_eq!(out[0].concurrency, ConcurrencyClass::Safe);
    }

    #[test]
    fn non_explore_profile_unchanged() {
        let mut a = item("a", TodoStatus::Pending);
        a.concurrency = ConcurrencyClass::Exclusive;
        let out = apply_explore_profile_safe_default("code", vec![a]);
        assert_eq!(out[0].concurrency, ConcurrencyClass::Exclusive);
    }

    fn ip(id: &str, class: ConcurrencyClass) -> TodoItemInput {
        let mut i = item(id, TodoStatus::InProgress);
        i.concurrency = class;
        i
    }

    #[test]
    fn safe_conflicts_with_exclusive_other() {
        // Another agent holding Exclusive blocks ALL InProgress here.
        let items = vec![ip("a", ConcurrencyClass::Safe)];
        let others = vec![("other".into(), "x".into(), ConcurrencyClass::Exclusive)];
        let r = validate_concurrency_cross_agent(&items, &others);
        assert!(matches!(r, Err(CodingTodoError::ConcurrencyViolation { .. })));
    }

    #[test]
    fn safe_never_conflicts_with_non_exclusive() {
        let items = vec![ip("a", ConcurrencyClass::Safe)];
        let others = vec![("other".into(), "x".into(), ConcurrencyClass::Sequential)];
        assert!(validate_concurrency_cross_agent(&items, &others).is_ok());
    }

    #[test]
    fn exclusive_conflicts_with_anything() {
        let items = vec![ip("a", ConcurrencyClass::Exclusive)];
        let others = vec![("other".into(), "x".into(), ConcurrencyClass::Safe)];
        let r = validate_concurrency_cross_agent(&items, &others);
        assert!(matches!(r, Err(CodingTodoError::ConcurrencyViolation { .. })));
    }

    #[test]
    fn sequential_conflicts_with_sequential() {
        let items = vec![ip("a", ConcurrencyClass::Sequential)];
        let others = vec![("other".into(), "x".into(), ConcurrencyClass::Sequential)];
        let r = validate_concurrency_cross_agent(&items, &others);
        assert!(matches!(r, Err(CodingTodoError::ConcurrencyViolation { .. })));
    }

    #[test]
    fn sequential_doesnt_conflict_with_safe() {
        let items = vec![ip("a", ConcurrencyClass::Sequential)];
        let others = vec![("other".into(), "x".into(), ConcurrencyClass::Safe)];
        assert!(validate_concurrency_cross_agent(&items, &others).is_ok());
    }

    #[test]
    fn plan_mode_off_allows_anything() {
        let items = vec![item("a", TodoStatus::InProgress), item("b", TodoStatus::Done)];
        assert!(validate_plan_mode_pending_only(false, &items).is_ok());
    }

    #[test]
    fn plan_mode_on_rejects_in_progress() {
        let items = vec![item("a", TodoStatus::InProgress)];
        let r = validate_plan_mode_pending_only(true, &items);
        assert!(matches!(r, Err(CodingTodoError::PlanModeNonPendingStatus { .. })));
    }

    #[test]
    fn plan_mode_on_allows_pending() {
        let items = vec![item("a", TodoStatus::Pending), item("b", TodoStatus::Pending)];
        assert!(validate_plan_mode_pending_only(true, &items).is_ok());
    }

    fn blocked_with(id: &str) -> TodoItemInput {
        let mut i = item(id, TodoStatus::Blocked);
        i.blocked_reason = Some("waiting on x".into());
        i
    }

    #[test]
    fn no_blocked_items_allows_through() {
        let items = vec![item("a", TodoStatus::Pending)];
        let ctx = ValidationContext {
            agent_id: "root",
            agent_profile: "root",
            plan_mode_active: false,
            previous_anti_passivity_violation: true,
            same_turn_user_msg_emitted: false,
            other_agents_in_progress: &[],
        };
        assert!(validate_anti_passivity(&items, &ctx).is_ok());
    }

    #[test]
    fn first_violation_allowed_no_prior() {
        let items = vec![blocked_with("a")];
        let ctx = ValidationContext {
            agent_id: "root",
            agent_profile: "root",
            plan_mode_active: false,
            previous_anti_passivity_violation: false,
            same_turn_user_msg_emitted: false,
            other_agents_in_progress: &[],
        };
        assert!(validate_anti_passivity(&items, &ctx).is_ok());
    }

    #[test]
    fn second_violation_no_msg_rejected() {
        let items = vec![blocked_with("a")];
        let ctx = ValidationContext {
            agent_id: "root",
            agent_profile: "root",
            plan_mode_active: false,
            previous_anti_passivity_violation: true,
            same_turn_user_msg_emitted: false,
            other_agents_in_progress: &[],
        };
        let r = validate_anti_passivity(&items, &ctx);
        assert!(matches!(r, Err(CodingTodoError::BlockedItemMissingUserMessage { .. })));
    }

    #[test]
    fn user_msg_clears_violation() {
        let items = vec![blocked_with("a")];
        let ctx = ValidationContext {
            agent_id: "root",
            agent_profile: "root",
            plan_mode_active: false,
            previous_anti_passivity_violation: true,
            same_turn_user_msg_emitted: true,
            other_agents_in_progress: &[],
        };
        assert!(validate_anti_passivity(&items, &ctx).is_ok());
    }

    #[test]
    fn validate_write_happy_path() {
        let a = item("a", TodoStatus::Pending);
        let mut b = item("b", TodoStatus::InProgress);
        b.concurrency = ConcurrencyClass::Sequential;
        let ctx = ValidationContext {
            agent_id: "root",
            agent_profile: "root",
            plan_mode_active: false,
            previous_anti_passivity_violation: false,
            same_turn_user_msg_emitted: false,
            other_agents_in_progress: &[],
        };
        let result = validate_write(vec![a, b], &ctx);
        assert!(result.is_ok(), "expected ok, got {:?}", result);
    }

    #[test]
    fn validate_write_explore_profile_forces_safe() {
        let mut a = item("a", TodoStatus::InProgress);
        a.concurrency = ConcurrencyClass::Exclusive;
        let ctx = ValidationContext {
            agent_id: "explore_1",
            agent_profile: "explore",
            plan_mode_active: false,
            previous_anti_passivity_violation: false,
            same_turn_user_msg_emitted: false,
            other_agents_in_progress: &[("other".into(), "x".into(), ConcurrencyClass::Sequential)],
        };
        let out = validate_write(vec![a], &ctx).unwrap();
        // Class was forced to Safe, so no conflict
        assert_eq!(out[0].concurrency, ConcurrencyClass::Safe);
    }
}
