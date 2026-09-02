//! Pure tool-call → entity-update intent projection (EUPI).
//!
//! Callers supply tool name + action only. Intents always use id `"*"`
//! (kind-level invalidation). Does not emit events or own session context.

use desktop_shared::types::EntityKind;

/// Broadcast sentinel for kind-level invalidation (not a concrete entity id).
pub const WILDCARD_ID: &str = "*";

/// Projected refresh intent — distinct from [`crate::EntityUpdate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityUpdateIntent {
    pub kind: EntityKind,
    pub id: &'static str,
}

struct ProjectionEntry {
    tool_name: &'static str,
    kinds: &'static [EntityKind],
    read_only_actions: &'static [&'static str],
}

const TASKS_READ_ONLY: &[&str] = &[
    "list",
    "show",
    "summary",
    "tree",
    "search",
    "list_recurring",
    "search-semantic",
    "search-hybrid",
    "query",
    "status",
    "stats",
    "get",
];

/// Shared chat/MCP parity cases: `(tool, action, expected kinds)`.
pub const PARITY_CASES: &[(&str, Option<&str>, &[EntityKind])] = &[
    ("tasks", Some("create"), &[EntityKind::Task]),
    ("tasks", Some("list"), &[]),
    ("notes", Some("create_note"), &[EntityKind::Note]),
    ("notes", Some("list_notes"), &[]),
    (
        "okr",
        Some("objective.create"),
        &[EntityKind::Objective, EntityKind::KeyResult],
    ),
    ("okr", Some("kr.show"), &[]),
    ("project", Some("update"), &[EntityKind::Project]),
    ("work_context", Some("rename"), &[EntityKind::Productivity]),
    ("unknown", Some("create"), &[]),
    ("get_status", Some("x"), &[]),
    ("agent", Some("x"), &[]),
];

static PROJECTION_TABLE: &[ProjectionEntry] = &[
    ProjectionEntry {
        tool_name: "tasks",
        kinds: &[EntityKind::Task],
        read_only_actions: TASKS_READ_ONLY,
    },
    ProjectionEntry {
        tool_name: "todo",
        kinds: &[EntityKind::Task],
        read_only_actions: TASKS_READ_ONLY,
    },
    ProjectionEntry {
        tool_name: "project",
        kinds: &[EntityKind::Project],
        read_only_actions: &[
            "list", "show", "tasks", "search", "get", "query", "status", "stats",
        ],
    },
    ProjectionEntry {
        tool_name: "area",
        kinds: &[EntityKind::Area],
        read_only_actions: &["list", "show", "search", "get", "query", "status", "stats"],
    },
    ProjectionEntry {
        tool_name: "okr",
        kinds: &[EntityKind::Objective, EntityKind::KeyResult],
        read_only_actions: &[
            "objective.list",
            "objective.show",
            "kr.list",
            "kr.show",
            "list",
            "show",
            "get",
            "search",
            "query",
            "status",
            "stats",
        ],
    },
    ProjectionEntry {
        tool_name: "notes",
        kinds: &[EntityKind::Note],
        read_only_actions: &[
            "list_notes",
            "get_note",
            "search_notes",
            "list_notebooks",
            "list_archived",
            "get_backlinks",
            "list_inbox",
            "list",
            "show",
            "get",
            "search",
            "query",
            "status",
            "stats",
            "search-semantic",
            "search-hybrid",
        ],
    },
    ProjectionEntry {
        tool_name: "productivity",
        kinds: &[EntityKind::Productivity],
        read_only_actions: &[
            "focus_status",
            "activity_today",
            "activity_summary",
            "activity_week",
            "activity_score",
            "activity_compare",
            "check_goals",
            "list_goals",
            "list_categories",
            "activity_export",
            "list",
            "show",
            "get",
            "search",
            "query",
            "status",
            "stats",
        ],
    },
    ProjectionEntry {
        tool_name: "work_context",
        kinds: &[EntityKind::Productivity],
        read_only_actions: &["list", "show", "search", "get", "query", "status", "stats"],
    },
];

/// Classify a successful tool call into zero or more kind-level intents.
///
/// Missing / empty / unknown actions produce intents when a projection entry
/// exists. Tools without an entry produce none.
pub fn project_entity_update(tool_name: &str, action: Option<&str>) -> Vec<EntityUpdateIntent> {
    let Some(entry) = PROJECTION_TABLE.iter().find(|e| e.tool_name == tool_name) else {
        return Vec::new();
    };

    let is_read_only = action
        .map(|a| !a.is_empty() && entry.read_only_actions.contains(&a))
        .unwrap_or(false);

    if is_read_only {
        return Vec::new();
    }

    entry
        .kinds
        .iter()
        .copied()
        .map(|kind| EntityUpdateIntent {
            kind,
            id: WILDCARD_ID,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds_of(tool: &str, action: Option<&str>) -> Vec<EntityKind> {
        project_entity_update(tool, action)
            .into_iter()
            .map(|i| i.kind)
            .collect()
    }

    #[test]
    fn project_entity_update_no_entry_returns_empty() {
        assert!(project_entity_update("unknown_tool", Some("create")).is_empty());
    }

    #[test]
    fn project_entity_update_read_only_returns_empty() {
        assert!(project_entity_update("tasks", Some("list")).is_empty());
        assert!(project_entity_update("tasks", Some("list_recurring")).is_empty());
        assert!(project_entity_update("notes", Some("list_notes")).is_empty());
        assert!(project_entity_update("okr", Some("objective.list")).is_empty());
        assert!(project_entity_update("productivity", Some("focus_status")).is_empty());
    }

    #[test]
    fn project_entity_update_mismatch_verbs_are_read_only_when_listed() {
        for action in [
            "show",
            "status",
            "stats",
            "query",
            "search-semantic",
            "search-hybrid",
        ] {
            assert!(
                project_entity_update("tasks", Some(action)).is_empty(),
                "tasks/{action} should be read-only"
            );
        }
    }

    #[test]
    fn project_entity_update_mutating_returns_wildcard_intents() {
        let intents = project_entity_update("tasks", Some("create"));
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].kind, EntityKind::Task);
        assert_eq!(intents[0].id, WILDCARD_ID);
    }

    #[test]
    fn project_entity_update_missing_or_unknown_action_mutates() {
        assert_eq!(kinds_of("tasks", None), vec![EntityKind::Task]);
        assert_eq!(kinds_of("tasks", Some("")), vec![EntityKind::Task]);
        assert_eq!(
            kinds_of("tasks", Some("brand_new_write")),
            vec![EntityKind::Task]
        );
    }

    #[test]
    fn project_entity_update_okr_is_multi_kind() {
        assert_eq!(
            kinds_of("okr", Some("objective.create")),
            vec![EntityKind::Objective, EntityKind::KeyResult]
        );
        assert!(project_entity_update("okr", Some("kr.list")).is_empty());
    }

    #[test]
    fn project_entity_update_todo_aliases_tasks() {
        assert_eq!(kinds_of("todo", Some("update")), vec![EntityKind::Task]);
    }

    /// Shared chat/MCP parity fixture (same name+action ⇒ same kinds).
    #[test]
    fn project_entity_update_parity_fixture() {
        for (tool, action, expected) in PARITY_CASES {
            assert_eq!(
                kinds_of(tool, *action).as_slice(),
                *expected,
                "parity {tool:?} {action:?}"
            );
        }
    }
}
