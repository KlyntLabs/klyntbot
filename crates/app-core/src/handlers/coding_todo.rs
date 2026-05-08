//! App-core handlers for coding todo operations.

use approval::CodingApprovalPolicy;
use bus::domain_events::TodoEvent;
use common::{KlyntbotError, Result};
use feature_coding_todo::types::{TodoItem, TodoItemInput};
use feature_coding_todo::validation::{validate_write, ValidationContext};
use feature_coding_todo::view::{CodingTodoView, PlanModeView};
use std::collections::HashMap;
use tracing::instrument;

use crate::state::AppCore;

/// Hard-coded agent ID for user-driven plan edits (single-user context).
const USER_AGENT_ID: &str = "root";

impl AppCore {
    #[instrument(skip(self), err)]
    pub async fn coding_todo_get(&self, thread_id: &str) -> Result<CodingTodoView> {
        let rows = self
            .repos
            .coding_todo
            .list_for_thread(thread_id)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        let mut agents: HashMap<String, Vec<TodoItem>> = HashMap::new();
        let mut total_proposed = 0usize;

        // Single DashMap lookup — clone the Arc so the shard lock is released.
        let policy_lock = self.coding_policies.get(thread_id).map(|r| r.clone());
        let plan_session_filter: Option<String> = policy_lock
            .as_ref()
            .and_then(|lock| lock.read().plan_session_id().map(|s| s.to_string()));

        for row in rows {
            let parsed: Vec<TodoItem> = serde_json::from_str(&row.items_json).unwrap_or_default();
            if let Some(filter) = &plan_session_filter {
                if row.proposed_in_plan_session.as_deref() == Some(filter.as_str()) {
                    total_proposed += parsed.len();
                }
            }
            agents.insert(row.agent_id, parsed);
        }

        let plan_mode_state = policy_lock.and_then(|lock| {
            let p = lock.read();
            let plan_session_id = p.plan_session_id()?;
            let plan_file_slug = p.plan_file_slug()?;
            let plan_file_path = p.plan_file_path()?;
            Some(PlanModeView {
                plan_session_id: plan_session_id.to_string(),
                plan_file_slug: plan_file_slug.to_string(),
                plan_file_path: plan_file_path.to_path_buf(),
                proposed_item_count: total_proposed,
            })
        });

        Ok(CodingTodoView {
            agents,
            plan_mode_state,
        })
    }

    #[instrument(skip(self), err)]
    pub async fn coding_plan_ratify(
        &self,
        thread_id: &str,
        plan_session_id: &str,
    ) -> Result<CodingTodoView> {
        // 1. Verify policy.
        let lock = self
            .coding_policies
            .get(thread_id)
            .ok_or_else(|| KlyntbotError::StorageNotFound(format!("no policy for {thread_id}")))?
            .clone();
        {
            let p = lock.read();
            match &*p {
                CodingApprovalPolicy::PlanMode {
                    plan_session_id: p_id,
                    ..
                } if p_id == plan_session_id => {}
                _ => {
                    return Err(KlyntbotError::NotImplemented(
                        "plan-session mismatch".into(),
                    ));
                }
            }
        }

        // 2. Diff snapshot vs final.
        let snapshot = self.plan_snapshots.get(plan_session_id).map(|r| r.clone());
        let rows = self
            .repos
            .coding_todo
            .list_for_thread(thread_id)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        let final_items: Vec<TodoItem> = rows
            .iter()
            .filter(|r| r.proposed_in_plan_session.as_deref() == Some(plan_session_id))
            .flat_map(|r| serde_json::from_str::<Vec<TodoItem>>(&r.items_json).unwrap_or_default())
            .collect();
        let (ratified, edited, removed) =
            super::coding_plan::compute_ratify_counts(snapshot.as_deref(), &final_items);

        // 3. Clear tags, swap policy, drop snapshot.
        self.repos
            .coding_todo
            .clear_plan_session_tag(thread_id, plan_session_id)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        *lock.write() = self.default_coding_policy();
        self.plan_snapshots.remove(plan_session_id);

        // 4. Events.
        if let Some(bus) = &self.domain_event_bus {
            bus.publish_todo(TodoEvent::PlanRatified {
                thread_id: thread_id.into(),
                plan_session_id: plan_session_id.into(),
                ratified_count: ratified,
                user_edited_count: edited,
                user_removed_count: removed,
                timestamp: jiff::Timestamp::now(),
            });
        }
        self.event_emitter.emit_event(
            "coding:todos_updated",
            serde_json::json!({ "thread_id": thread_id }),
        );
        self.event_emitter
            .emit_event("coding:plan_exited", serde_json::json!(thread_id));

        self.inject_one_shot_reminder(
            thread_id,
            &format!("Plan ratified by user. {ratified} items accepted, {edited} edited, {removed} removed. Executing now."),
        );

        // 5. Build view from rows already fetched — avoids a second DB round-trip.
        let mut agents: HashMap<String, Vec<TodoItem>> = HashMap::new();
        for row in rows {
            let parsed: Vec<TodoItem> = serde_json::from_str(&row.items_json).unwrap_or_default();
            agents.insert(row.agent_id, parsed);
        }
        Ok(CodingTodoView {
            agents,
            plan_mode_state: None,
        })
    }

    #[instrument(skip(self), err)]
    pub async fn coding_plan_user_edit(
        &self,
        thread_id: &str,
        plan_session_id: &str,
        items_json: &str,
    ) -> Result<CodingTodoView> {
        self.assert_plan_mode(thread_id, plan_session_id)?;

        let inputs: Vec<TodoItemInput> = serde_json::from_str(items_json)
            .map_err(|e| KlyntbotError::NotImplemented(format!("items_json: {e}")))?;

        // Validate via the existing validator with plan_mode_active=true.
        let ctx = ValidationContext {
            agent_id: USER_AGENT_ID,
            agent_profile: USER_AGENT_ID,
            plan_mode_active: true,
            previous_anti_passivity_violation: false,
            same_turn_user_msg_emitted: true, // user is editing — no anti-passivity nag
            other_agents_in_progress: &[],
        };
        let validated = validate_write(inputs, &ctx)
            .map_err(|e| KlyntbotError::NotImplemented(e.to_string()))?;

        // Materialize as TodoItem with current timestamps.
        let now = jiff::Timestamp::now();
        let materialized: Vec<TodoItem> = validated
            .into_iter()
            .map(|i| TodoItem {
                id: i
                    .id
                    .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string()),
                title: i.title,
                status: i.status,
                concurrency: i.concurrency,
                blocked_reason: i.blocked_reason,
                blocked_by: i.blocked_by,
                delegated_to: i.delegated_to,
                created_at: now,
                updated_at: now,
            })
            .collect();

        let json = serde_json::to_string(&materialized)
            .map_err(|e| KlyntbotError::Storage(format!("serialize items: {e}")))?;

        self.repos
            .coding_todo
            .upsert(thread_id, USER_AGENT_ID, &json, Some(plan_session_id))
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        self.event_emitter.emit_event(
            "coding:todos_updated",
            serde_json::json!({ "thread_id": thread_id }),
        );
        self.coding_todo_get(thread_id).await
    }

    #[instrument(skip(self), err)]
    pub async fn coding_plan_user_remove(
        &self,
        thread_id: &str,
        plan_session_id: &str,
        item_ids: &[String],
    ) -> Result<CodingTodoView> {
        self.assert_plan_mode(thread_id, plan_session_id)?;

        let row = self
            .repos
            .coding_todo
            .get(thread_id, USER_AGENT_ID)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?
            .ok_or_else(|| KlyntbotError::StorageNotFound("no plan items to remove".into()))?;
        let parsed: Vec<TodoItem> = serde_json::from_str(&row.items_json).unwrap_or_default();
        let item_ids_set: std::collections::HashSet<&str> =
            item_ids.iter().map(|s| s.as_str()).collect();
        let remaining: Vec<TodoItem> = parsed
            .into_iter()
            .filter(|i| !item_ids_set.contains(i.id.as_str()))
            .collect();
        let json = serde_json::to_string(&remaining)
            .map_err(|e| KlyntbotError::Storage(format!("serialize: {e}")))?;
        self.repos
            .coding_todo
            .upsert(thread_id, USER_AGENT_ID, &json, Some(plan_session_id))
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        self.event_emitter.emit_event(
            "coding:todos_updated",
            serde_json::json!({ "thread_id": thread_id }),
        );
        self.coding_todo_get(thread_id).await
    }

    fn assert_plan_mode(&self, thread_id: &str, plan_session_id: &str) -> Result<()> {
        let lock = self
            .coding_policies
            .get(thread_id)
            .ok_or_else(|| KlyntbotError::StorageNotFound(format!("no policy for {thread_id}")))?;
        let policy = lock.read();
        match &*policy {
            CodingApprovalPolicy::PlanMode {
                plan_session_id: p_id,
                ..
            } if p_id == plan_session_id => Ok(()),
            _ => Err(KlyntbotError::NotImplemented(
                "not in plan mode for this session".into(),
            )),
        }
    }

    pub(crate) fn default_coding_policy(&self) -> CodingApprovalPolicy {
        // Best-effort default — used when a thread had no entry before /plan.
        // TODO: cache empty CompiledRules in a static once the type supports it.
        CodingApprovalPolicy::Default {
            allow: approval::coding_policy::CompiledRules::compile(&[]).unwrap(),
            deny: approval::coding_policy::CompiledRules::compile(&[]).unwrap(),
            ask: approval::coding_policy::CompiledRules::compile(&[]).unwrap(),
            default_if_no_match: config::schema::DefaultPolicy::Ask,
        }
    }
}
