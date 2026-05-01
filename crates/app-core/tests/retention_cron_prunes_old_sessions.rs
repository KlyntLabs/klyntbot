use app_core::AppCore;
use jiff::Timestamp;

async fn insert_session(core: &AppCore, key: &str, pinned: bool, created_at: Timestamp) {
    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(key)
    .bind(serde_json::json!({}))
    .bind(created_at.as_millisecond())
    .bind(created_at.as_millisecond())
    .bind(pinned as i32)
    .execute(core.repos.pool())
    .await
    .unwrap();
}

async fn session_exists(core: &AppCore, key: &str) -> bool {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE key = ?")
        .bind(key)
        .fetch_one(core.repos.pool())
        .await
        .unwrap();
    count > 0
}

#[tokio::test]
async fn retention_cron_prunes_old_unpinned_sessions() {
    let dir = tempfile::TempDir::new().unwrap();
    let core = AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap();
    // Insert a 100-day-old session
    insert_session(
        &core,
        "old",
        false,
        jiff::Timestamp::from_millisecond(Timestamp::now().as_millisecond() - 100 * 86400 * 1000).unwrap(),
    )
    .await;
    // Insert a 100-day-old pinned session
    insert_session(
        &core,
        "old-pinned",
        true,
        jiff::Timestamp::from_millisecond(Timestamp::now().as_millisecond() - 100 * 86400 * 1000).unwrap(),
    )
    .await;
    // Insert a recent session
    insert_session(
        &core,
        "recent",
        false,
        jiff::Timestamp::from_millisecond(Timestamp::now().as_millisecond() - 10 * 86400 * 1000).unwrap(),
    )
    .await;

    core.run_session_retention_pass().await.unwrap();

    assert!(
        !session_exists(&core, "old").await,
        "old session should be pruned"
    );
    assert!(
        session_exists(&core, "old-pinned").await,
        "pinned survives"
    );
    assert!(
        session_exists(&core, "recent").await,
        "recent survives"
    );
}
