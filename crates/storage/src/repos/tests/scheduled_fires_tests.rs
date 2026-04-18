use crate::pool::StoragePool;
use crate::repos::scheduled_fires::ScheduledFiresRepo;
use crate::rows::scheduled_fire::ScheduledFireRow;
use tools_core::FeatureMigration;

fn sf(id: &str, at: i64, prefix: Option<&str>) -> ScheduledFireRow {
    ScheduledFireRow {
        id: id.into(),
        fire_at_ms: at,
        kind: "task_alarm".into(),
        ref_id: Some("task_1".into()),
        payload: serde_json::json!({}),
        dedup_prefix: prefix.map(String::from),
        fired: false,
        firing_started_at_ms: None,
        fired_at_ms: None,
        created_at_ms: 0,
    }
}

async fn setup() -> ScheduledFiresRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(
        pool.inner(),
        &[FeatureMigration {
            feature_name: "scheduling".into(),
            version: 1,
            description: "scheduled_fires".into(),
            sql: include_str!("../../../../scheduling/migrations/001_scheduled_fires.sql").into(),
        }],
    )
    .await
    .unwrap();
    ScheduledFiresRepo::new(pool.inner().clone())
}

#[tokio::test]
async fn insert_then_next_pending_returns_earliest() {
    let repo = setup().await;
    repo.insert(&sf("a", 2000, None)).await.unwrap();
    repo.insert(&sf("b", 1000, None)).await.unwrap();
    let next = repo.next_pending_fire_at_ms().await.unwrap();
    assert_eq!(next, Some(1000));
}

#[tokio::test]
async fn cancel_by_prefix_deletes_only_matching_pending() {
    let repo = setup().await;
    repo.insert(&sf("a", 1000, Some("task:1:"))).await.unwrap();
    repo.insert(&sf("b", 2000, Some("task:2:"))).await.unwrap();
    let deleted = repo.cancel_by_prefix("task:1:").await.unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(repo.list_pending_up_to_ms(9999).await.unwrap().len(), 1);
}

#[tokio::test]
async fn two_phase_mark_firing_then_fired_is_idempotent() {
    let repo = setup().await;
    repo.insert(&sf("a", 1000, None)).await.unwrap();
    let claimed = repo.begin_firing("a", 1500).await.unwrap();
    assert!(claimed);
    let claimed_again = repo.begin_firing("a", 1600).await.unwrap();
    assert!(!claimed_again, "begin_firing must be idempotent");
    repo.mark_fired("a", 1700).await.unwrap();
    assert_eq!(repo.list_pending_up_to_ms(9999).await.unwrap().len(), 0);
}

#[tokio::test]
async fn list_in_flight_returns_rows_with_firing_started_but_not_fired() {
    let repo = setup().await;
    repo.insert(&sf("a", 1000, None)).await.unwrap();
    repo.begin_firing("a", 1500).await.unwrap();
    let in_flight = repo.list_in_flight().await.unwrap();
    assert_eq!(in_flight.len(), 1);
    assert_eq!(in_flight[0].id, "a");
}
