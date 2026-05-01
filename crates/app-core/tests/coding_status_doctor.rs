use app_core::AppCore;

async fn insert_session(core: &AppCore, key: &str, pinned: bool, created_at: jiff::Timestamp) {
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

#[tokio::test]
async fn coding_status_returns_defaults_for_missing_session() {
    let dir = tempfile::TempDir::new().unwrap();
    let core = AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap();
    let status = core.coding_status("nonexistent").await.unwrap();
    assert_eq!(status.mode, "chat");
    assert_eq!(status.profile, "curated");
    assert_eq!(status.total_cost_usd, 0.0);
    assert_eq!(status.total_tokens, 0);
    assert!(status.active_skills.is_empty());
}

#[tokio::test]
async fn coding_doctor_returns_five_checks() {
    let dir = tempfile::TempDir::new().unwrap();
    let core = AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap();
    let diag = core.coding_doctor().await.unwrap();
    assert_eq!(diag.items.len(), 5);
    let names: Vec<String> = diag.items.iter().map(|i| i.name.clone()).collect();
    assert!(names.contains(&"hooks.toml".into()));
    assert!(names.contains(&"starlark rules".into()));
    assert!(names.contains(&"sandbox".into()));
    assert!(names.contains(&"skill loader".into()));
    assert!(names.contains(&"coding-memory".into()));
}

#[tokio::test]
async fn coding_sessions_star_unstar_works() {
    let dir = tempfile::TempDir::new().unwrap();
    let core = AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap();
    // Seed a session
    insert_session(&core, "s1", false, jiff::Timestamp::now()).await;

    core.coding_sessions_star("s1").await.unwrap();
    let row = core.repos.sessions.get_session("s1").await.unwrap();
    assert!(row.pinned);

    core.coding_sessions_unstar("s1").await.unwrap();
    let row = core.repos.sessions.get_session("s1").await.unwrap();
    assert!(!row.pinned);
}

#[tokio::test]
async fn coding_resume_finds_by_prefix() {
    let dir = tempfile::TempDir::new().unwrap();
    let core = AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap();
    // Seed a session with title in metadata
    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("s1")
    .bind(serde_json::json!({"title": "my-feature-thread"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let result = core.coding_resume("my-feat").await.unwrap();
    assert_eq!(result.session_key, "s1");
    assert_eq!(result.title, "my-feature-thread");
}

#[tokio::test]
async fn coding_help_returns_catalog() {
    let dir = tempfile::TempDir::new().unwrap();
    let core = AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap();
    let entries = core.coding_help(None).await.unwrap();
    assert!(!entries.is_empty());
    let commands: Vec<String> = entries.iter().map(|e| e.command.clone()).collect();
    assert!(commands.contains(&"/status".into()));
    assert!(commands.contains(&"/doctor".into()));
    assert!(commands.contains(&"/help".into()));
}
