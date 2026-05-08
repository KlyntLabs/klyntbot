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

use crate::state::AppCore;
use approval::CodingApprovalPolicy;
use common::{KlyntbotError, Result};
use feature_coding_todo::util::kebab;
use feature_coding_todo::view::{CodingTodoView, PlanModeView};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::instrument;

impl AppCore {
    /// Enter plan mode for `thread_id`. Idempotent: if the thread is already
    /// in plan mode, returns the existing view without changes.
    #[instrument(skip(self), err)]
    pub async fn coding_plan_enter(&self, thread_id: &str) -> Result<CodingTodoView> {
        // 1. Idempotent short-circuit.
        if let Some(lock) = self.coding_policies.get(thread_id) {
            if lock.read().is_plan_mode() {
                return self.coding_todo_get(thread_id).await;
            }
        }

        // 2. Read session title (best-effort; fall back to "untitled-<uuid8>").
        let title = self
            .repos
            .sessions
            .get_title(thread_id)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let plan_session_id = uuid::Uuid::new_v4().as_simple().to_string();
        let date = jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::system())
            .strftime("%Y-%m-%d")
            .to_string();
        let slug_body = if title.is_empty() {
            format!("untitled-{}", &plan_session_id[..8])
        } else {
            kebab(&title)
        };
        let slug = format!("{date}-{slug_body}");

        // 3. Build paths.
        let plans_dir = self.config.read().await.data_dir_path().join("plans");
        tokio::fs::create_dir_all(&plans_dir)
            .await
            .map_err(KlyntbotError::Io)?;
        let plan_file_path: PathBuf = plans_dir.join(format!("{slug}.md"));

        // 4. Create stub if absent.
        if !plan_file_path.exists() {
            let stub = format!(
                "# Plan: {}\n\n**Created:** {}\n**Plan session:** {}\n\n## Goals\n\n## Approach\n\n## Tasks\n",
                if title.is_empty() { "Untitled" } else { &title },
                jiff::Timestamp::now()
                    .to_zoned(jiff::tz::TimeZone::system())
                    .strftime("%Y-%m-%d %H:%M %Z"),
                plan_session_id,
            );
            tokio::fs::write(&plan_file_path, stub)
                .await
                .map_err(KlyntbotError::Io)?;
        }

        // 5. Build the new policy by cloning rules from the current Default
        //    (or fallback config-derived) policy.
        let new_policy = self.build_plan_mode_policy(thread_id, &plan_session_id, &slug, &plan_file_path).await?;

        // 6. Snapshot empty items (LLM hasn't proposed yet); refresh after first propose.
        self.plan_snapshots.insert(plan_session_id.clone(), Vec::new());

        // 7. Swap policy.
        let lock = self
            .coding_policies
            .entry(thread_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(self.default_coding_policy())))
            .clone();
        *lock.write() = new_policy;

        // 8. Spawn untitled-rename watcher if title was empty.
        if title.is_empty() {
            self.spawn_untitled_rename_watcher(thread_id.to_string(), plan_session_id.clone(), plans_dir);
        }

        // 9. Emit events.
        if let Some(bus) = &self.domain_event_bus {
            bus.publish_todo(bus::domain_events::TodoEvent::PlanProposed {
                thread_id: thread_id.into(),
                plan_session_id: plan_session_id.clone(),
                item_ids: vec![],
                timestamp: jiff::Timestamp::now(),
            });
        }
        // UI event
        self.event_emitter.emit_event("coding:plan_entered", serde_json::json!(thread_id));

        self.coding_todo_get(thread_id).await
    }

    #[instrument(skip(self), err)]
    pub async fn coding_plan_cancel(&self, thread_id: &str) -> Result<CodingTodoView> {
        let lock = self.coding_policies.get(thread_id)
            .ok_or_else(|| KlyntbotError::StorageNotFound(format!("no policy for thread {thread_id}")))?
            .clone();

        let plan_session_id = {
            let policy = lock.read();
            let p_id = policy.plan_session_id().map(|s| s.to_string());
            p_id.ok_or_else(|| KlyntbotError::NotImplemented("not in plan mode".into()))?
        };

        // Soft-delete plan-tagged rows.
        self.repos.coding_todo.delete_plan_session(thread_id, &plan_session_id).await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        // Swap to Default.
        *lock.write() = self.default_coding_policy();

        // Drop snapshot.
        self.plan_snapshots.remove(&plan_session_id);

        // Emit events.
        if let Some(bus) = &self.domain_event_bus {
            bus.publish_todo(bus::domain_events::TodoEvent::PlanCancelled {
                thread_id: thread_id.into(),
                plan_session_id,
                timestamp: jiff::Timestamp::now(),
            });
        }
        self.event_emitter.emit_event("coding:todos_updated", serde_json::json!({ "thread_id": thread_id }));
        self.event_emitter.emit_event("coding:plan_exited", serde_json::json!(thread_id));

        self.coding_todo_get(thread_id).await
    }

    async fn build_plan_mode_policy(
        &self,
        _thread_id: &str,
        plan_session_id: &str,
        plan_file_slug: &str,
        plan_file_path: &std::path::Path,
    ) -> Result<CodingApprovalPolicy> {
        let cfg = self.config.read().await;
        let perms = cfg.coding.permissions.clone();
        let base = CodingApprovalPolicy::compile(&perms)
            .map_err(|e| KlyntbotError::Config(common::ConfigError::Invalid(e)))?;
        let (allow, deny, ask, default_if_no_match) = match base {
            CodingApprovalPolicy::Default { allow, deny, ask, default_if_no_match } => {
                (allow, deny, ask, default_if_no_match)
            }
            _ => unreachable!("compile always returns Default"),
        };
        Ok(CodingApprovalPolicy::PlanMode {
            plan_session_id: plan_session_id.into(),
            plan_file_slug: plan_file_slug.into(),
            plan_file_path: plan_file_path.to_path_buf(),
            allow, deny, ask, default_if_no_match,
        })
    }

    fn spawn_untitled_rename_watcher(&self, thread_id: String, plan_session_id: String, plans_dir: std::path::PathBuf) {
        let policies = self.coding_policies.clone();
        let event_emitter = self.event_emitter.clone();
        let sessions_repo = self.repos.sessions.clone();
        tokio::spawn(async move {
            // TODO: subscribe to thread title updates and rename plan file when title arrives.
            // For now, the watcher is a stub that does nothing.
            let _ = (thread_id, plan_session_id, plans_dir, policies, event_emitter, sessions_repo);
        });
    }
}
