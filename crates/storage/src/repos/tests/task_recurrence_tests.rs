use crate::pool::StoragePool;
use crate::repos::task_recurrence::TaskRecurrenceRepo;
use crate::rows::task_recurrence::TaskRecurrenceTemplateRow;
use tools_core::FeatureMigration;

async fn setup() -> TaskRecurrenceRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(
        pool.inner(),
        &[FeatureMigration {
            feature_name: "tasks".into(),
            version: 1,
            description: "tasks + alarms + recurrence".into(),
            sql: include_str!("../../../../feature-tasks/migrations/001_create_tasks.sql").into(),
        }],
    )
    .await
    .unwrap();
    TaskRecurrenceRepo::new(pool.inner().clone())
}

#[tokio::test]
async fn upsert_then_get_round_trips() {
    let repo = setup().await;
    let row = TaskRecurrenceTemplateRow {
        id: "tpl_1".into(),
        source_task_id: "task_master".into(),
        rrule: "FREQ=WEEKLY;BYDAY=MO".into(),
        iana_tz: "America/New_York".into(),
        materialize_ahead: 3,
        next_instance_at_ms: Some(1_800_000_000_000),
        last_instance_at_ms: None,
        until_at_ms: None,
        count_remaining: None,
        enabled: true,
        created_at_ms: 0,
    };
    repo.upsert(&row).await.unwrap();
    let got = repo.get("tpl_1").await.unwrap().expect("row must exist");
    assert_eq!(got.rrule, "FREQ=WEEKLY;BYDAY=MO");
    assert_eq!(got.materialize_ahead, 3);
    assert!(got.enabled);
}

#[tokio::test]
async fn upsert_updates_mutable_fields_on_conflict() {
    let repo = setup().await;
    let base = TaskRecurrenceTemplateRow {
        id: "tpl_1".into(),
        source_task_id: "task_master".into(),
        rrule: "FREQ=DAILY".into(),
        iana_tz: "UTC".into(),
        materialize_ahead: 3,
        next_instance_at_ms: None,
        last_instance_at_ms: None,
        until_at_ms: None,
        count_remaining: None,
        enabled: true,
        created_at_ms: 100,
    };
    repo.upsert(&base).await.unwrap();
    // Now update: different rrule, enabled false, different created_at (should NOT be overwritten).
    let updated = TaskRecurrenceTemplateRow {
        rrule: "FREQ=WEEKLY".into(),
        enabled: false,
        created_at_ms: 999,
        ..base.clone()
    };
    repo.upsert(&updated).await.unwrap();
    let got = repo.get("tpl_1").await.unwrap().unwrap();
    assert_eq!(got.rrule, "FREQ=WEEKLY");
    assert!(!got.enabled);
    assert_eq!(
        got.created_at_ms, 100,
        "created_at must NOT be overwritten by upsert"
    );
}

#[tokio::test]
async fn list_enabled_excludes_disabled_rows() {
    let repo = setup().await;
    let mut row = TaskRecurrenceTemplateRow {
        id: "tpl_on".into(),
        source_task_id: "t1".into(),
        rrule: "FREQ=DAILY".into(),
        iana_tz: "UTC".into(),
        materialize_ahead: 3,
        next_instance_at_ms: None,
        last_instance_at_ms: None,
        until_at_ms: None,
        count_remaining: None,
        enabled: true,
        created_at_ms: 0,
    };
    repo.upsert(&row).await.unwrap();
    row.id = "tpl_off".into();
    row.enabled = false;
    repo.upsert(&row).await.unwrap();

    let enabled = repo.list_enabled().await.unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id, "tpl_on");
}
