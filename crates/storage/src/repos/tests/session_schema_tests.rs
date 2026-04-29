use crate::pool::StoragePool;

#[tokio::test]
async fn sessions_table_has_new_coding_columns() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let conn = pool.connection().await.unwrap();

    let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('sessions')")
        .fetch_all(&conn)
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
async fn sessions_default_approval_mode_is_default() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let conn = pool.connection().await.unwrap();

    sqlx::query("INSERT INTO sessions (key) VALUES ('s1')")
        .execute(&conn)
        .await
        .unwrap();
    let val: String = sqlx::query_scalar("SELECT approval_mode FROM sessions WHERE key='s1'")
        .fetch_one(&conn)
        .await
        .unwrap();
    assert_eq!(val, "default");
}
