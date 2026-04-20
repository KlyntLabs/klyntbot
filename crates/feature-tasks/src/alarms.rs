//! Task alarm materialization — turns user/agent-supplied `AlarmSpec`s
//! into persisted `task_alarms` rows + `scheduled_fires` rows so the
//! TemporalScheduler will fire them.
//!
//! Spec: §3 (rule model), §4.1 (schema), §8.3 (`AlarmSpec` shape).
//!
//! Two entry points:
//! - [`materialize_for_task`] — write rules + fires for a single task.
//! - [`cancel_for_task`] — remove all rules + pending fires for a task,
//!   used before re-materialization on update or before delete.

use std::sync::Arc;

use jiff::{civil::Time as CivilTime, Span, Timestamp};
use scheduling::temporal::fire_store::{FireSpec, FireStore};
use scheduling::temporal::rules::{AlarmRule, RuleError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use storage::repos::task_alarms::TaskAlarmsRepo;
use storage::rows::task_alarm::TaskAlarmRow;
use thiserror::Error;
use uuid::Uuid;

/// JSON-facing alarm specification supplied by the agent or user.
///
/// Mirrors spec §8.3 with `serde(tag = "type")` so payloads match the
/// shape `{type: "relative_before", offset_secs: 3600}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlarmSpec {
    RelativeBefore {
        offset_secs: i64,
        #[serde(default)]
        channels: Option<Vec<String>>,
        #[serde(default)]
        priority: Option<String>,
    },
    CivilTime {
        day_offset: i32,
        time_of_day: String, // "HH:MM"
        #[serde(default)]
        timezone: Option<String>,
        #[serde(default)]
        channels: Option<Vec<String>>,
        #[serde(default)]
        priority: Option<String>,
    },
    Absolute {
        fire_at: Timestamp,
        #[serde(default)]
        channels: Option<Vec<String>>,
        #[serde(default)]
        priority: Option<String>,
    },
}

#[derive(Debug, Error)]
pub enum MaterializeError {
    #[error("invalid time_of_day '{0}', expected HH:MM")]
    InvalidTimeOfDay(String),
    #[error("rule computation failed: {0}")]
    Rule(#[from] RuleError),
    #[error("storage error: {0}")]
    Storage(#[from] storage::error::StorageError),
    #[error("scheduler error: {0}")]
    Scheduler(#[from] scheduling::error::SchedulerError),
}

/// Channel name → bit. Mirrors `notifications::channel::CHANNEL_*`.
fn channel_name_to_bit(name: &str) -> u32 {
    match name {
        "os_native" => 1,
        "tray" => 2,
        "telegram" => 4,
        "discord" => 8,
        "email" => 16,
        _ => 0,
    }
}

fn channels_to_mask(channels: &Option<Vec<String>>) -> i64 {
    match channels {
        None => 0,
        Some(list) => list.iter().fold(0u32, |m, n| m | channel_name_to_bit(n)) as i64,
    }
}

fn parse_hhmm(s: &str) -> Result<CivilTime, MaterializeError> {
    let mut parts = s.split(':');
    let h = parts
        .next()
        .and_then(|x| x.parse::<i8>().ok())
        .ok_or_else(|| MaterializeError::InvalidTimeOfDay(s.into()))?;
    let m = parts
        .next()
        .and_then(|x| x.parse::<i8>().ok())
        .ok_or_else(|| MaterializeError::InvalidTimeOfDay(s.into()))?;
    if !(0..24).contains(&h) || !(0..60).contains(&m) || parts.next().is_some() {
        return Err(MaterializeError::InvalidTimeOfDay(s.into()));
    }
    CivilTime::new(h, m, 0, 0).map_err(|_| MaterializeError::InvalidTimeOfDay(s.into()))
}

/// Convert an `AlarmSpec` into the storage row + the canonical `AlarmRule`.
fn spec_to_row_and_rule(
    spec: &AlarmSpec,
    task_id: &str,
    default_tz: &str,
    now_ms: i64,
) -> Result<(TaskAlarmRow, AlarmRule), MaterializeError> {
    let id = format!("alarm_{}", Uuid::new_v4().simple());
    let (rule, row) = match spec {
        AlarmSpec::RelativeBefore {
            offset_secs,
            channels,
            priority,
        } => {
            let rule = AlarmRule::RelativeBefore {
                offset: Span::new().seconds(*offset_secs),
            };
            let row = TaskAlarmRow {
                id: id.clone(),
                task_id: task_id.into(),
                rule_type: "relative_before".into(),
                offset_secs: Some(*offset_secs),
                day_offset: None,
                time_of_day: None,
                iana_tz: None,
                absolute_fire_at_ms: None,
                channel_mask: channels_to_mask(channels),
                priority_override: priority.clone(),
                misfire_policy: None,
                grace_window_secs: None,
                created_at_ms: now_ms,
            };
            (rule, row)
        }
        AlarmSpec::CivilTime {
            day_offset,
            time_of_day,
            timezone,
            channels,
            priority,
        } => {
            let tz = timezone.clone().unwrap_or_else(|| default_tz.to_string());
            let civil = parse_hhmm(time_of_day)?;
            let rule = AlarmRule::CivilTimeOnDayOffset {
                day_offset: *day_offset,
                time_of_day: civil,
                iana_tz: tz.clone(),
            };
            let row = TaskAlarmRow {
                id: id.clone(),
                task_id: task_id.into(),
                rule_type: "civil_time".into(),
                offset_secs: None,
                day_offset: Some(*day_offset as i64),
                time_of_day: Some(time_of_day.clone()),
                iana_tz: Some(tz),
                absolute_fire_at_ms: None,
                channel_mask: channels_to_mask(channels),
                priority_override: priority.clone(),
                misfire_policy: None,
                grace_window_secs: None,
                created_at_ms: now_ms,
            };
            (rule, row)
        }
        AlarmSpec::Absolute {
            fire_at,
            channels,
            priority,
        } => {
            let rule = AlarmRule::Absolute { fire_at: *fire_at };
            let row = TaskAlarmRow {
                id: id.clone(),
                task_id: task_id.into(),
                rule_type: "absolute".into(),
                offset_secs: None,
                day_offset: None,
                time_of_day: None,
                iana_tz: None,
                absolute_fire_at_ms: Some(fire_at.as_millisecond()),
                channel_mask: channels_to_mask(channels),
                priority_override: priority.clone(),
                misfire_policy: None,
                grace_window_secs: None,
                created_at_ms: now_ms,
            };
            (rule, row)
        }
    };
    Ok((row, rule))
}

/// Dedup prefix used for both `task_alarms` cancellation and
/// `scheduled_fires.dedup_prefix`. Stable across re-materialization.
pub fn task_alarm_dedup_prefix(task_id: &str) -> String {
    format!("task:{task_id}:alarm:")
}

/// Materialize an alarm spec list for a task.
///
/// Caller is responsible for invoking [`cancel_for_task`] first if this is
/// an update; this function is purely additive.
///
/// Returns the number of alarms successfully materialized. Specs that fail
/// to compute (e.g. `RelativeBefore` with no `due_date`) are skipped with a
/// warning rather than aborting the whole batch.
pub async fn materialize_for_task(
    task_id: &str,
    due_date: Option<Timestamp>,
    specs: &[AlarmSpec],
    default_tz: &str,
    alarms_repo: &TaskAlarmsRepo,
    fire_store: &Arc<FireStore>,
) -> Result<usize, MaterializeError> {
    let now_ms = Timestamp::now().as_millisecond();
    let mut created = 0usize;

    for spec in specs {
        let (row, rule) = spec_to_row_and_rule(spec, task_id, default_tz, now_ms)?;
        let fire_at = match rule.compute_fire_at(due_date, default_tz) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    "task_alarms: skipping rule for task {task_id}: {e} (spec={:?})",
                    spec
                );
                continue;
            }
        };

        // Persist the rule definition.
        alarms_repo.insert(&row).await?;

        // Schedule the fire. Payload mirrors what NotificationDispatcher reads.
        let payload = json!({
            "title": "Task reminder",
            "body": format!("Alarm for task {task_id}"),
            "channel_mask": row.channel_mask,
            "priority_override": row.priority_override,
            "task_id": task_id,
            "alarm_id": row.id,
        });
        fire_store
            .schedule(FireSpec {
                fire_at,
                kind: "task_alarm".into(),
                ref_id: Some(row.id.clone()),
                payload,
                dedup_prefix: Some(task_alarm_dedup_prefix(task_id)),
            })
            .await?;
        created += 1;
    }

    Ok(created)
}

/// Cancel every `task_alarms` row + pending `scheduled_fires` row for a task.
/// Idempotent.
pub async fn cancel_for_task(
    task_id: &str,
    alarms_repo: &TaskAlarmsRepo,
    fire_store: &Arc<FireStore>,
) -> Result<(), MaterializeError> {
    fire_store
        .cancel_by_prefix(&task_alarm_dedup_prefix(task_id))
        .await?;
    alarms_repo.delete_by_task(task_id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::pool::StoragePool;
    use storage::repos::scheduled_fires::ScheduledFiresRepo;

    async fn setup() -> (TaskAlarmsRepo, Arc<FireStore>, StoragePool) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Run feature-tasks migrations (provides task_alarms via 001_create_tasks.sql)
        // and scheduling migrations (provides scheduled_fires).
        // tasks table has FK to areas — relax via no-FK schema for the test.
        sqlx::query("CREATE TABLE IF NOT EXISTS task_alarms (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, rule_type TEXT NOT NULL, offset_secs INTEGER, day_offset INTEGER, time_of_day TEXT, iana_tz TEXT, absolute_fire_at_ms INTEGER, channel_mask INTEGER NOT NULL DEFAULT 0, priority_override TEXT, misfire_policy TEXT, grace_window_secs INTEGER, created_at_ms INTEGER NOT NULL, UNIQUE (task_id, rule_type, offset_secs, day_offset, time_of_day, absolute_fire_at_ms))")
            .execute(pool.inner())
            .await
            .unwrap();
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
        let alarms_repo = TaskAlarmsRepo::new(pool.inner().clone());
        let sf_repo = ScheduledFiresRepo::new(pool.inner().clone());
        let fire_store = Arc::new(FireStore::new(sf_repo));
        (alarms_repo, fire_store, pool)
    }

    #[tokio::test]
    async fn relative_before_creates_one_row_each() {
        let (alarms, fires, _pool) = setup().await;
        let due = Timestamp::now() + Span::new().hours(48);
        let specs = vec![
            AlarmSpec::RelativeBefore {
                offset_secs: 3600,
                channels: None,
                priority: None,
            },
            AlarmSpec::RelativeBefore {
                offset_secs: 86400,
                channels: Some(vec!["telegram".into(), "email".into()]),
                priority: Some("urgent".into()),
            },
        ];
        let n = materialize_for_task("t1", Some(due), &specs, "UTC", &alarms, &fires)
            .await
            .unwrap();
        assert_eq!(n, 2);
        assert_eq!(alarms.list_by_task("t1").await.unwrap().len(), 2);

        // Channel mask: telegram(4) + email(16) = 20.
        let rows = alarms.list_by_task("t1").await.unwrap();
        let urgent = rows
            .iter()
            .find(|r| r.priority_override.as_deref() == Some("urgent"))
            .unwrap();
        assert_eq!(urgent.channel_mask, 20);
    }

    #[tokio::test]
    async fn relative_without_due_skips_silently() {
        let (alarms, fires, _pool) = setup().await;
        let specs = vec![AlarmSpec::RelativeBefore {
            offset_secs: 60,
            channels: None,
            priority: None,
        }];
        let n = materialize_for_task("t2", None, &specs, "UTC", &alarms, &fires)
            .await
            .unwrap();
        // Skipped because compute_fire_at fails without due_date.
        assert_eq!(n, 0);
        assert_eq!(alarms.list_by_task("t2").await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn absolute_does_not_require_due_date() {
        let (alarms, fires, _pool) = setup().await;
        let when = Timestamp::now() + Span::new().minutes(10);
        let specs = vec![AlarmSpec::Absolute {
            fire_at: when,
            channels: None,
            priority: None,
        }];
        let n = materialize_for_task("t3", None, &specs, "UTC", &alarms, &fires)
            .await
            .unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn cancel_for_task_removes_rules_and_pending_fires() {
        let (alarms, fires, _pool) = setup().await;
        let due = Timestamp::now() + Span::new().hours(24);
        let specs = vec![AlarmSpec::RelativeBefore {
            offset_secs: 3600,
            channels: None,
            priority: None,
        }];
        materialize_for_task("t4", Some(due), &specs, "UTC", &alarms, &fires)
            .await
            .unwrap();
        assert_eq!(alarms.list_by_task("t4").await.unwrap().len(), 1);

        cancel_for_task("t4", &alarms, &fires).await.unwrap();
        assert_eq!(alarms.list_by_task("t4").await.unwrap().len(), 0);
    }

    #[test]
    fn parse_hhmm_accepts_valid_and_rejects_invalid() {
        assert!(parse_hhmm("09:00").is_ok());
        assert!(parse_hhmm("23:59").is_ok());
        assert!(parse_hhmm("0:0").is_ok());
        assert!(parse_hhmm("24:00").is_err());
        assert!(parse_hhmm("9:60").is_err());
        assert!(parse_hhmm("9:00:00").is_err());
        assert!(parse_hhmm("not-a-time").is_err());
    }

    #[test]
    fn alarm_spec_serde_round_trips() {
        let spec = AlarmSpec::RelativeBefore {
            offset_secs: 3600,
            channels: Some(vec!["tray".into()]),
            priority: None,
        };
        let s = serde_json::to_string(&spec).unwrap();
        assert!(s.contains("\"type\":\"relative_before\""));
        let back: AlarmSpec = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            AlarmSpec::RelativeBefore { offset_secs: 3600, .. }
        ));
    }
}
