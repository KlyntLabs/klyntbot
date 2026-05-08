//! Format the current todo list as a system-reminder block. Used by
//! compaction-aware re-injection and subagent context injection.

use crate::types::{TodoItem, TodoStatus};

pub struct RenderConfig<'a> {
    pub kind: ReminderKind,
    pub agent_label: &'a str, // shown in header
}

pub enum ReminderKind {
    /// "Current coding todo list" — the agent's own writable list.
    Own,
    /// "Parent agent's current plan (read-only context)" — for subagents.
    ParentReadOnly,
}

pub fn render_reminder(items: &[TodoItem], cfg: &RenderConfig<'_>) -> String {
    let header = match cfg.kind {
        ReminderKind::Own => format!(
            "Current coding todo list (auto-injected after compaction; agent: {}):",
            cfg.agent_label
        ),
        ReminderKind::ParentReadOnly => format!(
            "Parent agent's current plan (read-only context — your task is delegated from this list). \
             You cannot modify it; you maintain your own coding_todo list. (parent: {})",
            cfg.agent_label
        ),
    };

    use std::fmt::Write;

    let mut body: Vec<String> = Vec::new();
    body.push(header);
    for item in items {
        let mut line = format!(
            "- [{}] {} · {}",
            status_short(item.status),
            &item.id[..item.id.len().min(8)],
            item.title,
        );
        let _ = write!(line, " · concurrency={:?}", item.concurrency);
        if !item.blocked_by.is_empty() {
            let _ = write!(line, " · blocked_by={}", item.blocked_by.join(","));
        }
        if let Some(reason) = &item.blocked_reason {
            let _ = write!(line, " · reason=\"{}\"", reason);
        }
        if let Some(d) = &item.delegated_to {
            let _ = write!(line, " · delegated_to={}", d);
        }
        body.push(line);
    }

    let inner = body.join("\n");
    format!("<system-reminder>\n{}\n</system-reminder>", inner)
}

fn status_short(s: TodoStatus) -> &'static str {
    match s {
        TodoStatus::Pending => "pending",
        TodoStatus::InProgress => "in_progress",
        TodoStatus::Done => "done",
        TodoStatus::Blocked => "blocked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ConcurrencyClass;

    fn item(id: &str, status: TodoStatus, title: &str) -> TodoItem {
        let now = jiff::Timestamp::from_second(1_780_000_000).unwrap();
        TodoItem {
            id: id.into(),
            title: title.into(),
            status,
            concurrency: ConcurrencyClass::Sequential,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn render_own_includes_header_and_items() {
        let items = vec![
            item("01HX0000A", TodoStatus::InProgress, "do thing"),
            item("01HX0000B", TodoStatus::Pending, "do other"),
        ];
        let cfg = RenderConfig {
            kind: ReminderKind::Own,
            agent_label: "root",
        };
        let s = render_reminder(&items, &cfg);
        assert!(s.contains("<system-reminder>"));
        assert!(s.contains("</system-reminder>"));
        assert!(s.contains("agent: root"));
        assert!(s.contains("[in_progress]"));
        assert!(s.contains("do thing"));
        assert!(s.contains("[pending]"));
        assert!(s.contains("do other"));
    }

    #[test]
    fn render_parent_readonly_uses_different_header() {
        let items = vec![item("01HX0000A", TodoStatus::Pending, "x")];
        let cfg = RenderConfig {
            kind: ReminderKind::ParentReadOnly,
            agent_label: "root",
        };
        let s = render_reminder(&items, &cfg);
        assert!(s.contains("read-only context"));
        assert!(s.contains("parent: root"));
    }

    #[test]
    fn render_includes_blocked_reason() {
        let mut i = item("01HX0000A", TodoStatus::Blocked, "x");
        i.blocked_reason = Some("waiting on user".into());
        let cfg = RenderConfig {
            kind: ReminderKind::Own,
            agent_label: "root",
        };
        let s = render_reminder(&[i], &cfg);
        assert!(s.contains("reason=\"waiting on user\""));
    }
}
