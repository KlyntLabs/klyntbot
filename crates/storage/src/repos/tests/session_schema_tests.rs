use crate::pool::StoragePool;

#[tokio::test]
async fn sessions_table_has_new_coding_columns() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let inner = pool.inner().clone();

    let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&inner)
        .await
        .unwrap();

    for required in &[
        "cwd",
        "repo_id",
        "repo_branch",
        "tool_profile",
        "approval_mode",
        "total_cost_usd",
        "total_tokens",
        "parent_session_id",
    ] {
        assert!(
            cols.iter().any(|c| c == required),
            "expected column `{}` on sessions table; columns are: {:?}",
            required,
            cols,
        );
    }
}

#[tokio::test]
async fn sessions_has_phase4_columns() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let inner = pool.inner().clone();

    let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&inner)
        .await
        .unwrap();

    for required in &[
        "workspace_id",
        "forked_from_id",
        "summary_message_id",
        "ephemeral",
        "archived_at",
    ] {
        assert!(
            cols.iter().any(|c| c == required),
            "expected column `{}` on sessions table; columns are: {:?}",
            required,
            cols,
        );
    }
}

#[tokio::test]
async fn session_messages_has_phase4_columns() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let inner = pool.inner().clone();

    let cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('session_messages')")
            .fetch_all(&inner)
            .await
            .unwrap();

    for required in &["parts", "turn_id", "finish_reason"] {
        assert!(
            cols.iter().any(|c| c == required),
            "expected column `{}` on session_messages table; columns are: {:?}",
            required,
            cols,
        );
    }
}

#[tokio::test]
async fn sessions_default_approval_mode_is_default() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let inner = pool.inner().clone();

    sqlx::query("INSERT INTO sessions (key) VALUES ('s1')")
        .execute(&inner)
        .await
        .unwrap();
    let val: String = sqlx::query_scalar("SELECT approval_mode FROM sessions WHERE key='s1'")
        .fetch_one(&inner)
        .await
        .unwrap();
    assert_eq!(val, "default");
}
