use crate::pool::StoragePool;
use crate::repos::task_alarms::TaskAlarmsRepo;
use crate::rows::task_alarm::TaskAlarmRow;
use tools_core::FeatureMigration;

async fn setup() -> TaskAlarmsRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // connect_in_memory() already runs storage/001_initial.sql (areas, projects, etc.).
    // Now apply the feature-tasks migration which creates tasks, task_alarms, etc.
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

    // Seed a minimal area and task row required by the FK constraints.
    sqlx::query("INSERT INTO areas (id, name, created_at) VALUES ('area_1', 'Test Area', 0)")
        .execute(pool.inner())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO tasks (id, title, area_id, created_at, updated_at) \
         VALUES ('task_1', 'Test Task', 'area_1', 0, 0)",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    TaskAlarmsRepo::new(pool.inner().clone())
}

#[tokio::test]
async fn insert_and_list_by_task() {
    let repo = setup().await;
    let row = TaskAlarmRow {
        id: "a1".into(),
        task_id: "task_1".into(),
        rule_type: "relative_before".into(),
        offset_secs: Some(3600),
        day_offset: None,
        time_of_day: None,
        iana_tz: None,
        absolute_fire_at_ms: None,
        channel_mask: 0,
        priority_override: None,
        misfire_policy: None,
        grace_window_secs: None,
        created_at_ms: 0,
    };
    repo.insert(&row).await.unwrap();
    let listed = repo.list_by_task("task_1").await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "a1");
}

#[tokio::test]
async fn delete_by_task_cascade() {
    let repo = setup().await;
    repo.insert(&TaskAlarmRow {
        id: "a1".into(),
        task_id: "task_1".into(),
        rule_type: "relative_before".into(),
        offset_secs: Some(3600),
        day_offset: None,
        time_of_day: None,
        iana_tz: None,
        absolute_fire_at_ms: None,
        channel_mask: 0,
        priority_override: None,
        misfire_policy: None,
        grace_window_secs: None,
        created_at_ms: 0,
    })
    .await
    .unwrap();
    repo.delete_by_task("task_1").await.unwrap();
    assert!(repo.list_by_task("task_1").await.unwrap().is_empty());
}
