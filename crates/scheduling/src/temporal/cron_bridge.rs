//! Bridge between legacy `cron_jobs` (definition table) and `scheduled_fires` (firing table).
//!
//! For each enabled `cron_jobs` row, the bridge maintains exactly one pending
//! `scheduled_fires` row with `kind="cron_job"` and `ref_id=<cron_jobs.id>`.
//!
//! Called by the scheduler:
//!   - On startup via `reconcile_all()` — repair any inconsistency.
//!   - After each cron fire via `advance(job_id)` — insert the next run.
//!   - On cron job enable/disable/delete — `reconcile_all()` repairs.
//!
//! ⚠️ CHRONO BOUNDARY: the upstream `cron` crate requires `chrono::TimeZone`
//! at its API. Chrono is private to this module — downstream code sees only
//! `jiff::Timestamp`.

use cron::Schedule;
use jiff::Timestamp;
use std::str::FromStr;
use storage::repos::cron::CronRepo;
use storage::rows::cron::CronJobRow;

use crate::error::SchedulerError;
use crate::temporal::fire_store::{FireSpec, FireStore};

#[derive(Debug, Clone)]
pub struct CronBridge {
    cron: CronRepo,
    fires: FireStore,
}

impl CronBridge {
    pub fn new(cron: CronRepo, fires: FireStore) -> Self {
        Self { cron, fires }
    }

    /// Ensure every enabled `cron_jobs` row has exactly one pending
    /// `scheduled_fires` row. Clears pending rows for disabled jobs.
    /// Called on startup and whenever `cron_jobs` changes.
    pub async fn reconcile_all(&self) -> Result<(), SchedulerError> {
        let jobs = self.cron.list().await?;
        for job in jobs {
            if job.enabled {
                self.ensure_scheduled(&job).await?;
            } else {
                self.fires.cancel_by_kind_ref("cron_job", &job.id).await?;
            }
        }
        Ok(())
    }

    /// Called after a cron fire dispatches: compute the next run and insert a
    /// fresh pending row. No-op if the job has been disabled since.
    pub async fn advance(&self, job_id: &str) -> Result<(), SchedulerError> {
        // If the job was deleted between fire and advance, no-op instead of
        // propagating NotFound — the scheduler dispatch path must not fault on
        // a user-initiated delete race.
        let job = match self.cron.get(job_id).await {
            Ok(j) => j,
            Err(storage::error::StorageError::NotFound(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if job.enabled {
            self.ensure_scheduled(&job).await?;
        }
        Ok(())
    }

    /// Insert a pending fire for this job, clearing any stale pending row first.
    /// Idempotent — safe to call multiple times.
    async fn ensure_scheduled(&self, job: &CronJobRow) -> Result<(), SchedulerError> {
        self.fires.cancel_by_kind_ref("cron_job", &job.id).await?;
        let next = self.next_fire_for(job)?;
        self.fires
            .schedule(FireSpec {
                fire_at: next,
                kind: "cron_job".into(),
                ref_id: Some(job.id.clone()),
                payload: serde_json::json!({ "job": job.id, "name": job.name }),
                dedup_prefix: Some(format!("cron:{}:", job.id)),
            })
            .await?;
        Ok(())
    }

    /// Compute the next fire time for a cron job. Uses the `cron` crate at a
    /// chrono boundary hidden inside this function.
    fn next_fire_for(&self, job: &CronJobRow) -> Result<Timestamp, SchedulerError> {
        let cron_expr = job
            .schedule
            .get("cron")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                SchedulerError::InvalidState(format!(
                    "cron job {} has no 'cron' field in schedule",
                    job.id
                ))
            })?;
        let tz_name = job
            .schedule
            .get("tz")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC");
        let schedule = Schedule::from_str(cron_expr).map_err(|e| {
            SchedulerError::InvalidState(format!("invalid cron '{cron_expr}': {e}"))
        })?;

        let tz: chrono_tz::Tz = tz_name
            .parse()
            .map_err(|e| SchedulerError::InvalidState(format!("bad tz {tz_name}: {e}")))?;
        let now_utc = chrono::Utc::now();
        let now_tz = now_utc.with_timezone(&tz);
        let next_tz = schedule.after(&now_tz).next().ok_or_else(|| {
            SchedulerError::InvalidState(format!(
                "cron schedule for job {} yielded no next run",
                job.id
            ))
        })?;
        Timestamp::from_millisecond(next_tz.timestamp_millis())
            .map_err(|_| SchedulerError::InvalidState("cron next out of jiff range".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::pool::StoragePool;
    use storage::repos::cron::CronRepo;
    use storage::repos::scheduled_fires::ScheduledFiresRepo;
    use storage::rows::cron::CronJobRow;

    async fn setup() -> (CronBridge, CronRepo, ScheduledFiresRepo) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // connect_in_memory runs initial migrations (includes cron_jobs table).
        // Apply feature-scheduling migration for scheduled_fires.
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
        let cron = CronRepo::new(pool.inner().clone());
        let sf = ScheduledFiresRepo::new(pool.inner().clone());
        let bridge = CronBridge::new(
            cron.clone(),
            crate::temporal::fire_store::FireStore::new(sf.clone()),
        );
        (bridge, cron, sf)
    }

    fn make_cron_job(id: &str, enabled: bool, cron_expr: &str) -> CronJobRow {
        CronJobRow {
            id: id.into(),
            name: format!("job-{id}"),
            enabled,
            origin: "user".into(),
            schedule: serde_json::json!({ "cron": cron_expr, "tz": "UTC" }),
            payload: serde_json::json!({}),
            next_run_at_ms: None,
            last_run_at_ms: None,
            last_status: None,
            last_error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
            delete_after_run: false,
            intent_window: None,
            intent_pending_since_ms: None,
        }
    }

    #[tokio::test]
    async fn reconcile_creates_pending_fire_for_enabled_job() {
        let (bridge, cron, sf) = setup().await;
        cron.upsert(&make_cron_job("j1", true, "0 9 * * * * *"))
            .await
            .unwrap();
        bridge.reconcile_all().await.unwrap();
        let pending = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, "cron_job");
        assert_eq!(pending[0].ref_id.as_deref(), Some("j1"));
    }

    #[tokio::test]
    async fn reconcile_does_not_duplicate_when_called_twice() {
        let (bridge, cron, sf) = setup().await;
        cron.upsert(&make_cron_job("j1", true, "0 9 * * * * *"))
            .await
            .unwrap();
        bridge.reconcile_all().await.unwrap();
        bridge.reconcile_all().await.unwrap();
        let pending = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[tokio::test]
    async fn reconcile_removes_pending_for_disabled_job() {
        let (bridge, cron, sf) = setup().await;
        cron.upsert(&make_cron_job("j1", true, "0 9 * * * * *"))
            .await
            .unwrap();
        bridge.reconcile_all().await.unwrap();
        assert_eq!(sf.list_pending_up_to_ms(i64::MAX).await.unwrap().len(), 1);

        // Disable the job; reconcile should clear its pending fire.
        cron.upsert(&make_cron_job("j1", false, "0 9 * * * * *"))
            .await
            .unwrap();
        bridge.reconcile_all().await.unwrap();
        assert_eq!(sf.list_pending_up_to_ms(i64::MAX).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn advance_no_ops_when_job_deleted() {
        let (bridge, cron, _sf) = setup().await;
        cron.upsert(&make_cron_job("j1", true, "0 9 * * * * *"))
            .await
            .unwrap();
        bridge.reconcile_all().await.unwrap();

        // Delete the job AFTER it's been reconciled (simulating a race where the
        // scheduler has already fired and is about to call advance).
        cron.delete("j1").await.unwrap();

        // advance must not error.
        bridge
            .advance("j1")
            .await
            .expect("advance on deleted job should no-op");
    }
}
