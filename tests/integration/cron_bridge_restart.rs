//! Task 4.18 — CronBridge restart reconciliation.
//!
//! Verifies that `CronBridge::reconcile_all` correctly (re-)establishes
//! exactly one pending `scheduled_fires` row per enabled cron_job after a
//! simulated restart, even when a row was left in-flight (i.e.
//! `firing_started_at_ms` set, `fired=0`) at shutdown.

use scheduling::temporal::cron_bridge::CronBridge;
use scheduling::temporal::fire_store::FireStore;
use storage::pool::StoragePool;
use storage::repos::cron::CronRepo;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use storage::rows::cron::CronJobRow;
use tools_core::FeatureMigration;

async fn setup() -> (StoragePool, CronRepo, ScheduledFiresRepo) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // connect_in_memory runs the base migrations (includes cron_jobs).
    // Add scheduled_fires on top.
    StoragePool::run_feature_migrations(
        pool.inner(),
        &[FeatureMigration {
            feature_name: "scheduling".into(),
            version: 1,
            description: "scheduled_fires".into(),
            sql: include_str!("../../crates/scheduling/migrations/001_scheduled_fires.sql").into(),
        }],
    )
    .await
    .unwrap();
    let cron = CronRepo::new(pool.inner().clone());
    let sf = ScheduledFiresRepo::new(pool.inner().clone());
    (pool, cron, sf)
}

fn make_job(id: &str) -> CronJobRow {
    CronJobRow {
        id: id.into(),
        name: format!("job-{id}"),
        enabled: true,
        origin: "user".into(),
        // Every minute — always has a next run.
        schedule: serde_json::json!({ "cron": "0 * * * * * *", "tz": "UTC" }),
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

/// First-boot reconcile: 3 enabled cron_jobs → 3 pending scheduled_fires rows.
#[tokio::test]
async fn reconcile_all_creates_one_fire_per_enabled_job() {
    let (pool, cron, sf) = setup().await;
    let fire_store = FireStore::new(sf.clone());
    let bridge = CronBridge::new(cron.clone(), fire_store);

    for id in ["c1", "c2", "c3"] {
        cron.upsert(&make_job(id)).await.unwrap();
    }

    bridge.reconcile_all().await.unwrap();

    let pending = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert_eq!(
        pending.len(),
        3,
        "one pending fire per enabled cron_job after first reconcile"
    );
    assert!(
        pending.iter().all(|r| r.kind == "cron_job"),
        "all rows have kind=cron_job"
    );

    // Each job id appears exactly once.
    let mut ref_ids: Vec<_> = pending
        .iter()
        .filter_map(|r| r.ref_id.as_deref())
        .collect();
    ref_ids.sort_unstable();
    assert_eq!(ref_ids, ["c1", "c2", "c3"]);

    drop(pool); // suppress unused warning
}

/// Restart reconciliation: simulate a mid-fire crash by setting
/// `firing_started_at_ms` on one row without marking it `fired=1`, then
/// call reconcile_all on a fresh CronBridge. Expect exactly 1 pending fire
/// per enabled job (no duplicates, in-flight row replaced with fresh one).
#[tokio::test]
async fn restart_reconcile_replaces_in_flight_row_without_duplication() {
    let (pool, cron, sf) = setup().await;
    let fire_store = FireStore::new(sf.clone());

    for id in ["r1", "r2", "r3"] {
        cron.upsert(&make_job(id)).await.unwrap();
    }

    // ── First scheduler boot ──────────────────────────────────────────────
    let bridge1 = CronBridge::new(cron.clone(), fire_store.clone());
    bridge1.reconcile_all().await.unwrap();

    let before = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert_eq!(before.len(), 3, "3 pending fires after first reconcile");

    // Simulate "in-flight crash": set firing_started_at_ms on one row but
    // leave fired=0. The scheduler would normally set this during begin_firing.
    let in_flight_id = &before[0].id;
    let now_ms = jiff::Timestamp::now().as_millisecond();
    sqlx::query(
        "UPDATE scheduled_fires SET firing_started_at_ms = ?1 WHERE id = ?2",
    )
    .bind(now_ms)
    .bind(in_flight_id)
    .execute(pool.inner())
    .await
    .unwrap();

    // ── Simulated restart ─────────────────────────────────────────────────
    // Drop bridge1 (scheduler gone). Build a fresh CronBridge on the same pool.
    drop(bridge1);

    let bridge2 = CronBridge::new(cron.clone(), fire_store.clone());
    bridge2.reconcile_all().await.unwrap();

    // After reconcile: ensure_scheduled cancels all pending (including
    // in-flight, fired=0) rows and inserts fresh ones.
    // Expected: exactly 3 pending fires, one per enabled job.
    let after = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert_eq!(
        after.len(),
        3,
        "exactly one pending fire per enabled job after restart reconcile (no duplicates)"
    );

    // No in-flight rows remain (they were cleared by reconcile).
    let in_flight = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert!(
        in_flight
            .iter()
            .all(|r| r.firing_started_at_ms.is_none()),
        "no in-flight rows remain after reconcile"
    );

    // Ref ids still one-per-job.
    let mut ref_ids: Vec<_> = after
        .iter()
        .filter_map(|r| r.ref_id.as_deref())
        .collect();
    ref_ids.sort_unstable();
    assert_eq!(ref_ids, ["r1", "r2", "r3"]);
}

/// Calling reconcile_all multiple times never creates duplicates.
#[tokio::test]
async fn repeated_reconcile_is_idempotent() {
    let (_pool, cron, sf) = setup().await;
    let fire_store = FireStore::new(sf.clone());
    let bridge = CronBridge::new(cron.clone(), fire_store);

    cron.upsert(&make_job("idem1")).await.unwrap();
    cron.upsert(&make_job("idem2")).await.unwrap();

    // Reconcile 4 times in a row.
    for _ in 0..4 {
        bridge.reconcile_all().await.unwrap();
    }

    let pending = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert_eq!(
        pending.len(),
        2,
        "reconcile is idempotent: still exactly 2 pending fires"
    );
}
