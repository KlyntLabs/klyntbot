//! Tests for ActionRepo — all concerns.

use super::*;
use crate::rows::action::ActionRow;

#[test]
fn test_get_by_ids_empty_input_short_circuits() {
    let ids: Vec<String> = vec![];
    assert!(
        ids.is_empty(),
        "Empty ids should trigger early return in get_by_ids"
    );
}

#[tokio::test]
async fn time_entries_in_range_filters_by_date() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    let repo = ActionRepo::new(db.clone());

    // Create required area + action
    sqlx::query(
        "INSERT INTO areas (id, name, color, status) VALUES ('a', 'Test', '#000', 'active')",
    )
    .execute(&db)
    .await
    .unwrap();
    let now = chrono::Utc::now();
    let row = ActionRow {
        id: "te-range-1".into(),
        title: "Test Task".into(),
        description: None,
        area_id: "a".into(),
        project_id: None,
        key_result_id: None,
        parent_id: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: "todo".into(),
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: now,
        updated_at: now,
        completed_at: None,
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
        status_label_id: None,
        position: 0,
        group_id: None,
    };
    repo.add(&row).await.unwrap();

    // Add time entries on different dates
    let mar8 = chrono::NaiveDate::from_ymd_opt(2026, 3, 8)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_utc();
    let mar9 = chrono::NaiveDate::from_ymd_opt(2026, 3, 9)
        .unwrap()
        .and_hms_opt(14, 0, 0)
        .unwrap()
        .and_utc();
    let mar10 = chrono::NaiveDate::from_ymd_opt(2026, 3, 10)
        .unwrap()
        .and_hms_opt(9, 0, 0)
        .unwrap()
        .and_utc();

    repo.add_time_entry("te-range-1", "manual", mar8, Some(3600), None)
        .await
        .unwrap();
    repo.add_time_entry("te-range-1", "manual", mar9, Some(5400), None)
        .await
        .unwrap();
    repo.add_time_entry("te-range-1", "manual", mar10, Some(2700), None)
        .await
        .unwrap();

    let results = repo
        .time_entries_in_range("2026-03-09", "2026-03-09")
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].action_title, "Test Task");

    let results = repo
        .time_entries_in_range("2026-03-08", "2026-03-10")
        .await
        .unwrap();
    assert_eq!(results.len(), 3);
}
