//! `AlarmTool` — standalone reminder tool. Spec §8.2.
//!
//! Backed by `scheduled_fires` rows with `kind = "standalone_alarm"`.
//! Each alarm has a stable `alarm_id` (UUID) and dedup_prefix
//! `standalone:{alarm_id}:` so snooze + cancel are cancel-by-prefix +
//! re-insert.

use std::sync::Arc;

use async_trait::async_trait;
use bus::{AlarmEvent, DomainEventBus};
use common::{Result, ToolError};
use jiff::{SignedDuration, Timestamp};
use notifications::channel::names_to_mask;
use scheduling::temporal::fire_store::{FireSpec, FireStore};
use serde_json::{json, Value};
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use tools_core::{approval_class::ApprovalClass, ParamExtractor, RoutingContext, Tool};
use tracing::warn;
use uuid::Uuid;

const KIND: &str = "standalone_alarm";

fn dedup_prefix(alarm_id: &str) -> String {
    format!("standalone:{alarm_id}:")
}

/// Parse a duration like "10m", "1h", "2d", "30s", or an integer (seconds).
fn parse_duration(s: &str) -> Result<SignedDuration> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ToolError::InvalidParams("empty duration".into()).into());
    }
    let (num_part, unit) = if let Some(stripped) = s.strip_suffix(|c: char| c.is_ascii_alphabetic())
    {
        (stripped, &s[stripped.len()..])
    } else {
        (s, "s")
    };
    let n: i64 = num_part
        .parse()
        .map_err(|_| ToolError::InvalidParams(format!("invalid duration number: {s}")))?;
    let secs = match unit {
        "s" | "" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86400,
        other => {
            return Err(ToolError::InvalidParams(format!(
                "unknown duration unit '{other}', expected s|m|h|d"
            ))
            .into())
        }
    };
    Ok(SignedDuration::from_secs(secs))
}

/// Standalone alarm tool — free-floating reminders not anchored to any task.
pub struct AlarmTool {
    fire_store: Arc<FireStore>,
    sf_repo: ScheduledFiresRepo,
    domain_bus: Option<Arc<DomainEventBus>>,
}

impl AlarmTool {
    pub fn new(fire_store: Arc<FireStore>, sf_repo: ScheduledFiresRepo) -> Self {
        Self {
            fire_store,
            sf_repo,
            domain_bus: None,
        }
    }

    pub fn with_domain_bus(mut self, bus: Arc<DomainEventBus>) -> Self {
        self.domain_bus = Some(bus);
        self
    }

    async fn handle_create(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let message = p.required_str("message")?.to_string();
        let priority = p.optional_str("priority")?.map(String::from);
        let channels: Option<Vec<String>> = p.optional_array("channels")?.map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

        let fire_at = if let Some(s) = p.optional_str("fire_at")? {
            s.parse::<Timestamp>()
                .map_err(|e| ToolError::InvalidParams(format!("invalid fire_at '{s}': {e}")))?
        } else if let Some(s) = p.optional_str("relative_duration")? {
            Timestamp::now() + parse_duration(s)?
        } else {
            return Err(ToolError::InvalidParams(
                "AlarmTool.create requires either fire_at or relative_duration".into(),
            )
            .into());
        };

        let alarm_id = format!("alarm_{}", Uuid::new_v4().simple());
        let mask = channels.as_deref().map(names_to_mask).unwrap_or(0);
        let payload = json!({
            "title": "Reminder",
            "body": message,
            "channel_mask": mask,
            "priority_override": priority,
            "alarm_id": alarm_id,
        });
        let fire_id = self
            .fire_store
            .schedule(FireSpec {
                fire_at,
                kind: KIND.into(),
                ref_id: Some(alarm_id.clone()),
                payload,
                dedup_prefix: Some(dedup_prefix(&alarm_id)),
            })
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("schedule failed: {e}")))?;

        Ok(format!(
            "Alarm scheduled (id: {alarm_id}, fire_id: {fire_id}, fire_at: {fire_at})"
        ))
    }

    async fn handle_list(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let cutoff = i64::MAX;
        let rows = self
            .sf_repo
            .list_pending_with_kind_up_to_ms(cutoff, KIND)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("list failed: {e}")))?;
        if rows.is_empty() {
            return Ok("No pending alarms.".into());
        }
        let mut out = String::from("Pending alarms:\n");
        for r in &rows {
            let fire_at = Timestamp::from_millisecond(r.fire_at_ms)
                .map(|t| t.to_string())
                .unwrap_or_else(|_| format!("ms={}", r.fire_at_ms));
            let body = serde_json::from_value::<Value>(r.payload.clone())
                .ok()
                .and_then(|v| v.get("body").and_then(|b| b.as_str()).map(String::from))
                .unwrap_or_default();
            let alarm_id = r.ref_id.as_deref().unwrap_or("?");
            out.push_str(&format!("- [{alarm_id}] {fire_at}: {body}\n"));
        }
        Ok(out)
    }

    async fn handle_cancel(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let alarm_id = p.required_str("id")?;
        let removed = self
            .fire_store
            .cancel_by_prefix(&dedup_prefix(alarm_id))
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("cancel failed: {e}")))?;
        if removed == 0 {
            return Err(
                ToolError::ExecutionFailed(format!("no pending alarm with id {alarm_id}")).into(),
            );
        }
        if let Some(ref bus) = self.domain_bus {
            bus.publish_alarm(AlarmEvent::AlarmCancelled {
                fire_id: alarm_id.into(),
                reason: "user_cancel".into(),
            });
        }
        Ok(format!(
            "Cancelled alarm {alarm_id} ({removed} pending fires removed)"
        ))
    }

    async fn handle_snooze(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let alarm_id = p.required_str("id")?;
        let duration = parse_duration(p.required_str("duration")?)?;

        // Find the existing pending row to preserve its payload.
        let rows = self
            .sf_repo
            .list_pending_with_kind_up_to_ms(i64::MAX, KIND)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("snooze lookup failed: {e}")))?;
        let existing = rows
            .into_iter()
            .find(|r| r.ref_id.as_deref() == Some(alarm_id))
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("no pending alarm with id {alarm_id}"))
            })?;

        let new_fire_at = Timestamp::now() + duration;
        // Cancel the old, schedule the new with the same payload + ref_id.
        self.fire_store
            .cancel_by_prefix(&dedup_prefix(alarm_id))
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("snooze cancel failed: {e}")))?;
        let new_fire_id = self
            .fire_store
            .schedule(FireSpec {
                fire_at: new_fire_at,
                kind: KIND.into(),
                ref_id: Some(alarm_id.into()),
                payload: existing.payload,
                dedup_prefix: Some(dedup_prefix(alarm_id)),
            })
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("snooze reschedule failed: {e}")))?;

        if let Some(ref bus) = self.domain_bus {
            bus.publish_alarm(AlarmEvent::AlarmSnoozed {
                fire_id: new_fire_id.clone(),
                new_fire_at_ms: new_fire_at.as_millisecond(),
            });
        }
        Ok(format!(
            "Snoozed alarm {alarm_id} → {new_fire_at} (new fire_id: {new_fire_id})"
        ))
    }
}

#[async_trait]
impl Tool for AlarmTool {
    fn name(&self) -> &str {
        "alarm"
    }

    fn exposure_policy(&self) -> tools_core::ExposurePolicy {
        tools_core::ExposurePolicy {
            mcp: tools_core::McpExposure::Default,
            ..Default::default()
        }
    }

    fn description(&self) -> &str {
        "Standalone reminders not tied to any task. Create with `fire_at` (ISO 8601) or \
         `relative_duration` (e.g. '10m', '1h', '2d'). Use 'tasks' tool with `alarms` \
         field for task-attached reminders instead."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "cancel", "snooze"],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Alarm id (cancel, snooze)" },
                "fire_at": { "type": "string", "description": "ISO 8601 timestamp (create); mutex with relative_duration" },
                "relative_duration": { "type": "string", "description": "e.g. '10m', '1h', '2d', '30s' (create); mutex with fire_at" },
                "message": { "type": "string", "description": "Reminder text (create)" },
                "channels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "channel names: os_native|tray|telegram|discord|email"
                },
                "priority": {
                    "type": "string",
                    "enum": ["normal", "urgent"],
                    "description": "Optional priority override"
                },
                "duration": { "type": "string", "description": "Snooze duration e.g. '10m' (snooze)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;
        match action {
            "create" => self.handle_create(&p).await,
            "list" => self.handle_list(&p).await,
            "cancel" => self.handle_cancel(&p).await,
            "snooze" => self.handle_snooze(&p).await,
            other => {
                warn!("AlarmTool: unknown action '{other}'");
                Err(ToolError::InvalidParams(format!("unknown action '{other}'")).into())
            }
        }
    }

    fn approval_class(&self, args: &Value) -> ApprovalClass {
        match args.get("action").and_then(|v| v.as_str()) {
            Some("create" | "snooze") => ApprovalClass::Sensitive,
            Some("cancel") => ApprovalClass::Destructive,
            _ => ApprovalClass::Safe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::pool::StoragePool;

    async fn setup() -> (AlarmTool, ScheduledFiresRepo) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../scheduling/migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        let sf = ScheduledFiresRepo::new(pool.inner().clone());
        let tool = AlarmTool::new(Arc::new(FireStore::new(sf.clone())), sf.clone());
        (tool, sf)
    }

    fn ctx() -> RoutingContext {
        RoutingContext::new(common::ChannelName::new("cli"), common::ChatId::new("test"))
    }

    #[tokio::test]
    async fn create_relative_then_list_finds_it() {
        let (tool, _) = setup().await;
        let r = tool
            .execute(
                json!({"action": "create", "relative_duration": "1h", "message": "drink water"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(r.contains("Alarm scheduled"));
        let list = tool
            .execute(json!({"action": "list"}), &ctx())
            .await
            .unwrap();
        assert!(list.contains("drink water"));
    }

    #[tokio::test]
    async fn create_requires_one_of_fire_at_or_relative() {
        let (tool, _) = setup().await;
        let err = tool
            .execute(
                json!({"action": "create", "message": "no time spec"}),
                &ctx(),
            )
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn cancel_removes_pending_fire() {
        let (tool, sf) = setup().await;
        let r = tool
            .execute(
                json!({"action": "create", "relative_duration": "1h", "message": "x"}),
                &ctx(),
            )
            .await
            .unwrap();
        // Extract the alarm_ id from the response.
        let id = r
            .split("id: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim();
        assert_eq!(sf.list_pending_up_to_ms(i64::MAX).await.unwrap().len(), 1);
        tool.execute(json!({"action": "cancel", "id": id}), &ctx())
            .await
            .unwrap();
        assert_eq!(sf.list_pending_up_to_ms(i64::MAX).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn snooze_replaces_pending_fire_with_later_one() {
        let (tool, sf) = setup().await;
        let r = tool
            .execute(
                json!({"action": "create", "relative_duration": "10m", "message": "y"}),
                &ctx(),
            )
            .await
            .unwrap();
        let id = r
            .split("id: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim();
        let before = sf.list_pending_up_to_ms(i64::MAX).await.unwrap()[0].fire_at_ms;
        tool.execute(
            json!({"action": "snooze", "id": id, "duration": "1h"}),
            &ctx(),
        )
        .await
        .unwrap();
        let after = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
        assert_eq!(after.len(), 1, "snooze keeps exactly one pending fire");
        assert!(after[0].fire_at_ms > before, "snooze pushed fire later");
    }

    #[test]
    fn parse_duration_units() {
        assert_eq!(
            parse_duration("30s").unwrap(),
            SignedDuration::from_secs(30)
        );
        assert_eq!(
            parse_duration("5m").unwrap(),
            SignedDuration::from_secs(300)
        );
        assert_eq!(
            parse_duration("2h").unwrap(),
            SignedDuration::from_secs(7200)
        );
        assert_eq!(
            parse_duration("3d").unwrap(),
            SignedDuration::from_secs(259_200)
        );
        assert_eq!(parse_duration("90").unwrap(), SignedDuration::from_secs(90));
        assert!(parse_duration("").is_err());
        assert!(parse_duration("nope").is_err());
        assert!(parse_duration("5x").is_err());
    }
}
