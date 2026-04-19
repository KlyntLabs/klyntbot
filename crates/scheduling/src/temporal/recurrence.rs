//! `RecurrenceEngine` — drives RRULE template materialization.
//!
//! Called when a `kind='recurrence_spawn'` scheduled_fire fires.
//! Implementations of `TemplateRepo` and `InstanceRepo` live in `feature-tasks`
//! (Task 4.3); this module only defines the engine + trait boundaries.

use std::sync::Arc;

use async_trait::async_trait;
use jiff::Timestamp;
use serde_json::json;

use crate::error::SchedulerError;
use crate::temporal::fire_store::{FireSpec, FireStore};
use crate::temporal::rrule::next_n_from_rrule_string;

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RecurrenceTemplate {
    pub id: String,
    pub source_task_id: String,
    /// RFC 5545 RRULE text (e.g. "FREQ=DAILY;INTERVAL=1").
    pub rrule: String,
    /// IANA timezone (e.g. "America/New_York").
    pub iana_tz: String,
    /// How many future instances to materialise at once. 0 = use engine default.
    pub materialize_ahead: u32,
    pub next_instance_at: Option<Timestamp>,
    pub until_at: Option<Timestamp>,
    pub count_remaining: Option<u32>,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Repository traits
// ---------------------------------------------------------------------------

#[async_trait]
pub trait TemplateRepo: Send + Sync {
    /// Returns the current template state. `count_remaining` MUST reflect the live value
    /// (not a cached snapshot) — the engine reads it before deciding whether to materialize.
    async fn get(&self, id: &str) -> anyhow::Result<Option<RecurrenceTemplate>>;
    async fn update_next_instance(
        &self,
        id: &str,
        next_at: Option<Timestamp>,
    ) -> anyhow::Result<()>;
    /// Decrement `count_remaining` by 1 and return the new value (None if column was NULL).
    async fn decrement_count(&self, id: &str) -> anyhow::Result<Option<u32>>;
    async fn disable(&self, id: &str) -> anyhow::Result<()>;
}

#[async_trait]
pub trait InstanceRepo: Send + Sync {
    /// Create an instance row and return its id.
    async fn create_instance(&self, template_id: &str, due_at: Timestamp)
        -> anyhow::Result<String>;
    /// Cancel all unfired instances for this template.
    async fn cancel_unfired_instances(&self, template_id: &str) -> anyhow::Result<()>;
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub struct RecurrenceEngine {
    store: Arc<FireStore>,
    template_repo: Arc<dyn TemplateRepo>,
    instance_repo: Arc<dyn InstanceRepo>,
    default_materialize_ahead: u32,
}

impl RecurrenceEngine {
    pub fn new(
        store: Arc<FireStore>,
        template_repo: Arc<dyn TemplateRepo>,
        instance_repo: Arc<dyn InstanceRepo>,
        default_materialize_ahead: u32,
    ) -> Self {
        Self {
            store,
            template_repo,
            instance_repo,
            default_materialize_ahead,
        }
    }

    /// Called when a `recurrence_spawn` alarm fires for `template_id`.
    pub async fn on_spawn(&self, template_id: &str, now: Timestamp) -> Result<(), SchedulerError> {
        let tmpl = match self
            .template_repo
            .get(template_id)
            .await
            .map_err(|e| SchedulerError::InvalidState(e.to_string()))?
        {
            Some(t) if t.enabled => t,
            _ => return Ok(()),
        };

        let ahead = if tmpl.materialize_ahead == 0 {
            self.default_materialize_ahead
        } else {
            tmpl.materialize_ahead
        };

        let cursor = tmpl.next_instance_at.unwrap_or(now);

        // Check count guard before doing anything.
        if matches!(tmpl.count_remaining, Some(0)) {
            self.template_repo
                .update_next_instance(template_id, None)
                .await
                .map_err(|e| SchedulerError::InvalidState(e.to_string()))?;
            return Ok(());
        }

        // Fetch enough candidates to cover `ahead` instances + 1 for next_spawn.
        // Uses the native `rrule` crate parser so the full RFC 5545 property set
        // is supported (BYSETPOS, BYYEARDAY, negative BYMONTHDAY, WKST, etc.).
        let candidates =
            next_n_from_rrule_string(&tmpl.rrule, &tmpl.iana_tz, cursor, ahead as usize + 1)
                .map_err(|e| SchedulerError::Rrule(e.to_string()))?;

        let mut last_materialized: Option<Timestamp> = None;

        for (idx, &next) in candidates.iter().enumerate() {
            if idx as u32 >= ahead {
                break;
            }

            // Respect UNTIL guard (inclusive per RFC 5545 §3.3.10: UNTIL date is included).
            if let Some(until) = tmpl.until_at {
                if next > until {
                    break;
                }
            }

            self.instance_repo
                .create_instance(template_id, next)
                .await
                .map_err(|e| SchedulerError::InvalidState(e.to_string()))?;

            // Decrement count if finite.
            if tmpl.count_remaining.is_some() {
                let new_count = self
                    .template_repo
                    .decrement_count(template_id)
                    .await
                    .map_err(|e| SchedulerError::InvalidState(e.to_string()))?;
                if new_count == Some(0) {
                    self.template_repo
                        .update_next_instance(template_id, None)
                        .await
                        .map_err(|e| SchedulerError::InvalidState(e.to_string()))?;
                    return Ok(());
                }
            }

            last_materialized = Some(next);
        }

        // Determine next_spawn: the first candidate that comes after all materialized ones.
        // If nothing was materialized (UNTIL already exhausted or count was 0), short-circuit
        // to None — calling the evaluator again would produce a timestamp past UNTIL and
        // cause an endless recurrence_spawn cycle.
        let next_spawn = if let Some(last) = last_materialized {
            candidates.into_iter().find(|t| *t > last)
        } else {
            None
        };

        self.template_repo
            .update_next_instance(template_id, next_spawn)
            .await
            .map_err(|e| SchedulerError::InvalidState(e.to_string()))?;

        if let Some(fire_at) = next_spawn {
            self.store
                .schedule(FireSpec {
                    fire_at,
                    kind: super::scheduler::RECURRENCE_SPAWN_KIND.into(),
                    ref_id: Some(template_id.to_string()),
                    payload: json!({}),
                    dedup_prefix: Some(format!("template:{template_id}:")),
                })
                .await?;
        }

        Ok(())
    }

    /// Disable a template and cancel all related pending work.
    pub async fn disable_template(&self, template_id: &str) -> Result<(), SchedulerError> {
        self.template_repo
            .disable(template_id)
            .await
            .map_err(|e| SchedulerError::InvalidState(e.to_string()))?;
        self.instance_repo
            .cancel_unfired_instances(template_id)
            .await
            .map_err(|e| SchedulerError::InvalidState(e.to_string()))?;
        self.store
            .cancel_by_prefix(&format!("template:{template_id}:"))
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Test mocks (accessible within the scheduling crate's test code)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod test_mocks {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub(crate) struct MockTemplateState {
        pub(crate) templates: HashMap<String, RecurrenceTemplate>,
        pub(crate) next_instance: HashMap<String, Option<Timestamp>>,
        pub(crate) disabled: Vec<String>,
        pub(crate) count_remaining: HashMap<String, Option<u32>>,
    }

    pub(crate) struct MockTemplateRepo(pub(crate) Mutex<MockTemplateState>);

    impl MockTemplateRepo {
        pub(crate) fn with(tmpl: RecurrenceTemplate) -> Self {
            let mut state = MockTemplateState::default();
            let count = tmpl.count_remaining;
            let id = tmpl.id.clone();
            let next = tmpl.next_instance_at;
            state.templates.insert(id.clone(), tmpl);
            state.count_remaining.insert(id.clone(), count);
            state.next_instance.insert(id, next);
            MockTemplateRepo(Mutex::new(state))
        }
    }

    #[async_trait]
    impl TemplateRepo for MockTemplateRepo {
        async fn get(&self, id: &str) -> anyhow::Result<Option<RecurrenceTemplate>> {
            let state = self.0.lock().unwrap();
            let mut t = state.templates.get(id).cloned();
            if let Some(ref mut tmpl) = t {
                tmpl.count_remaining = *state.count_remaining.get(id).unwrap_or(&None);
            }
            Ok(t)
        }

        async fn update_next_instance(
            &self,
            id: &str,
            next_at: Option<Timestamp>,
        ) -> anyhow::Result<()> {
            self.0
                .lock()
                .unwrap()
                .next_instance
                .insert(id.to_string(), next_at);
            Ok(())
        }

        async fn decrement_count(&self, id: &str) -> anyhow::Result<Option<u32>> {
            let mut state = self.0.lock().unwrap();
            let entry = state.count_remaining.entry(id.to_string()).or_insert(None);
            if let Some(ref mut c) = entry {
                *c = c.saturating_sub(1);
                Ok(Some(*c))
            } else {
                Ok(None)
            }
        }

        async fn disable(&self, id: &str) -> anyhow::Result<()> {
            let mut state = self.0.lock().unwrap();
            state.disabled.push(id.to_string());
            if let Some(t) = state.templates.get_mut(id) {
                t.enabled = false;
            }
            Ok(())
        }
    }

    #[derive(Default)]
    pub(crate) struct MockInstanceRepo(pub(crate) Mutex<Vec<(String, Timestamp)>>);

    #[async_trait]
    impl InstanceRepo for MockInstanceRepo {
        async fn create_instance(
            &self,
            template_id: &str,
            due_at: Timestamp,
        ) -> anyhow::Result<String> {
            self.0
                .lock()
                .unwrap()
                .push((template_id.to_string(), due_at));
            Ok(format!("inst_{}", due_at.as_millisecond()))
        }

        async fn cancel_unfired_instances(&self, _template_id: &str) -> anyhow::Result<()> {
            self.0.lock().unwrap().clear();
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use storage::pool::StoragePool;
    use storage::repos::scheduled_fires::ScheduledFiresRepo;
    use test_mocks::{MockInstanceRepo, MockTemplateRepo};

    // -----------------------------------------------------------------------
    // Helper: build a real in-memory FireStore (same pattern as fire_store tests)
    // -----------------------------------------------------------------------

    async fn make_fire_store() -> FireStore {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        FireStore::new(ScheduledFiresRepo::new(pool.inner().clone()))
    }

    fn ts_days_from_epoch(days: i64) -> Timestamp {
        Timestamp::from_millisecond(days * 86_400_000).unwrap()
    }

    // 2026-04-28 00:00:00 UTC
    fn april28() -> Timestamp {
        jiff::civil::date(2026, 4, 28)
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp()
    }

    // -----------------------------------------------------------------------
    // Test 1: UNTIL stops materialization
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn until_stops_materialization() {
        let fire_store = Arc::new(make_fire_store().await);
        let tmpl_id = "t1".to_string();

        let until = jiff::civil::date(2026, 5, 1)
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();

        let template = RecurrenceTemplate {
            id: tmpl_id.clone(),
            source_task_id: "task1".into(),
            rrule: "FREQ=DAILY".into(),
            iana_tz: "UTC".into(),
            materialize_ahead: 10,
            next_instance_at: Some(april28()),
            until_at: Some(until),
            count_remaining: None,
            enabled: true,
        };

        let tmpl_repo = Arc::new(MockTemplateRepo::with(template));
        let inst_repo = Arc::new(MockInstanceRepo::default());
        let engine = RecurrenceEngine::new(fire_store, tmpl_repo, inst_repo.clone(), 3);

        engine.on_spawn(&tmpl_id, april28()).await.unwrap();

        // Expect: 4/28, 4/29, 4/30, 5/1 — UNTIL is inclusive per RFC 5545 §3.3.10.
        let instances = inst_repo.0.lock().unwrap();
        assert_eq!(
            instances.len(),
            4,
            "expected 4 instances (4/28, 4/29, 4/30, 5/1), got {}",
            instances.len()
        );
        assert_eq!(instances[0].1.to_string(), "2026-04-28T00:00:00Z");
        assert_eq!(instances[1].1.to_string(), "2026-04-29T00:00:00Z");
        assert_eq!(instances[2].1.to_string(), "2026-04-30T00:00:00Z");
        assert_eq!(instances[3].1.to_string(), "2026-05-01T00:00:00Z"); // UNTIL inclusive per RFC 5545
    }

    // -----------------------------------------------------------------------
    // Test 2: count decrements to zero then halts
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn count_decrements_to_zero_then_halts() {
        let fire_store = Arc::new(make_fire_store().await);
        let tmpl_id = "t2".to_string();

        let template = RecurrenceTemplate {
            id: tmpl_id.clone(),
            source_task_id: "task2".into(),
            rrule: "FREQ=DAILY".into(),
            iana_tz: "UTC".into(),
            materialize_ahead: 10,
            next_instance_at: Some(april28()),
            until_at: None,
            count_remaining: Some(2),
            enabled: true,
        };

        let tmpl_repo = Arc::new(MockTemplateRepo::with(template));
        let inst_repo = Arc::new(MockInstanceRepo::default());
        let engine = RecurrenceEngine::new(fire_store, tmpl_repo.clone(), inst_repo.clone(), 3);

        engine.on_spawn(&tmpl_id, april28()).await.unwrap();

        let instances = inst_repo.0.lock().unwrap();
        assert_eq!(
            instances.len(),
            2,
            "expected 2 instances, got {}",
            instances.len()
        );
        drop(instances);

        // count_remaining should be 0
        let state = tmpl_repo.0.lock().unwrap();
        let remaining = state.count_remaining.get(&tmpl_id).copied().flatten();
        assert_eq!(remaining, Some(0));

        // update_next_instance(None) should have been called
        let next = state.next_instance.get(&tmpl_id).copied().flatten();
        assert!(
            next.is_none(),
            "next_instance should be None after exhaustion"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: disable cancels unfired + pending spawns
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn disable_cancels_unfired_and_pending_spawns() {
        let fire_store = Arc::new(make_fire_store().await);
        let tmpl_id = "t3".to_string();

        // Pre-populate a pending recurrence_spawn fire
        fire_store
            .schedule(FireSpec {
                fire_at: ts_days_from_epoch(1000),
                kind: "recurrence_spawn".into(),
                ref_id: Some(tmpl_id.clone()),
                payload: json!({}),
                dedup_prefix: Some(format!("template:{tmpl_id}:")),
            })
            .await
            .unwrap();

        let template = RecurrenceTemplate {
            id: tmpl_id.clone(),
            source_task_id: "task3".into(),
            rrule: "FREQ=DAILY".into(),
            iana_tz: "UTC".into(),
            materialize_ahead: 1,
            next_instance_at: None,
            until_at: None,
            count_remaining: None,
            enabled: true,
        };

        let tmpl_repo = Arc::new(MockTemplateRepo::with(template));
        // Pre-populate an instance
        let inst_repo = Arc::new(MockInstanceRepo::default());
        inst_repo
            .0
            .lock()
            .unwrap()
            .push(("t3".to_string(), ts_days_from_epoch(999)));

        let engine =
            RecurrenceEngine::new(fire_store.clone(), tmpl_repo.clone(), inst_repo.clone(), 3);

        engine.disable_template(&tmpl_id).await.unwrap();

        // Instances cleared — scope the lock so it's dropped before any await
        {
            let instances = inst_repo.0.lock().unwrap();
            assert_eq!(instances.len(), 0, "unfired instances should be cancelled");
        }

        // Pending fires with prefix cleared
        let pending = fire_store.list_due(ts_days_from_epoch(9999)).await.unwrap();
        let matching: Vec<_> = pending
            .iter()
            .filter(|r| {
                r.dedup_prefix
                    .as_deref()
                    .map(|p| p.starts_with(&format!("template:{tmpl_id}:")))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(matching.len(), 0, "pending spawn fires should be cancelled");

        // disable called on template
        let state = tmpl_repo.0.lock().unwrap();
        assert!(state.disabled.contains(&tmpl_id));
    }

    // -----------------------------------------------------------------------
    // Test 4: disabled template spawn is a no-op
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn disabled_template_spawn_is_noop() {
        let fire_store = Arc::new(make_fire_store().await);
        let tmpl_id = "t4".to_string();

        let template = RecurrenceTemplate {
            id: tmpl_id.clone(),
            source_task_id: "task4".into(),
            rrule: "FREQ=DAILY".into(),
            iana_tz: "UTC".into(),
            materialize_ahead: 5,
            next_instance_at: Some(april28()),
            until_at: None,
            count_remaining: None,
            enabled: false,
        };

        let tmpl_repo = Arc::new(MockTemplateRepo::with(template));
        let inst_repo = Arc::new(MockInstanceRepo::default());
        let engine = RecurrenceEngine::new(fire_store.clone(), tmpl_repo, inst_repo.clone(), 3);

        engine.on_spawn(&tmpl_id, april28()).await.unwrap();

        // Scope the lock so it drops before the await below
        {
            let instances = inst_repo.0.lock().unwrap();
            assert_eq!(
                instances.len(),
                0,
                "disabled template should create no instances"
            );
        }

        // No new spawn scheduled either
        let pending = fire_store.list_due(ts_days_from_epoch(9999)).await.unwrap();
        assert_eq!(
            pending.len(),
            0,
            "no spawn should be scheduled for disabled template"
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: native parser handles properties the old custom parser lacked
    //
    // BYMONTHDAY=-1 means "last day of month".  The custom parser treated -1
    // as an opaque i32 but had no RFC 5545 semantics — the rrule crate handles
    // it correctly.  Starting 2026-01-15 with UNTIL=2026-04-01 we expect
    // exactly 3 instances: Jan 31, Feb 28, Mar 31.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn unknown_rrule_property_is_respected() {
        let fire_store = Arc::new(make_fire_store().await);
        let tmpl_id = "t5".to_string();

        // cursor: 2026-01-15 00:00:00 UTC (before Jan 31)
        let cursor = jiff::civil::date(2026, 1, 15)
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();

        // UNTIL = 2026-04-01 00:00:00 UTC (exclusive of Apr 30)
        let until = jiff::civil::date(2026, 4, 1)
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();

        let template = RecurrenceTemplate {
            id: tmpl_id.clone(),
            source_task_id: "task5".into(),
            rrule: "FREQ=MONTHLY;BYMONTHDAY=-1".into(),
            iana_tz: "UTC".into(),
            materialize_ahead: 10,
            next_instance_at: Some(cursor),
            until_at: Some(until),
            count_remaining: None,
            enabled: true,
        };

        let tmpl_repo = Arc::new(MockTemplateRepo::with(template));
        let inst_repo = Arc::new(MockInstanceRepo::default());
        let engine = RecurrenceEngine::new(fire_store, tmpl_repo, inst_repo.clone(), 3);

        engine.on_spawn(&tmpl_id, cursor).await.unwrap();

        let instances = inst_repo.0.lock().unwrap();
        // Jan 31, Feb 28, Mar 31 — Apr 30 is past UNTIL so excluded
        assert_eq!(
            instances.len(),
            3,
            "expected 3 instances (Jan 31, Feb 28, Mar 31), got {}: {:?}",
            instances.len(),
            instances
                .iter()
                .map(|(_, ts)| ts.to_string())
                .collect::<Vec<_>>(),
        );
        assert_eq!(instances[0].1.to_string(), "2026-01-31T00:00:00Z"); // last day of Jan
        assert_eq!(instances[1].1.to_string(), "2026-02-28T00:00:00Z"); // last day of Feb (non-leap)
        assert_eq!(instances[2].1.to_string(), "2026-03-31T00:00:00Z"); // last day of Mar
    }
}
