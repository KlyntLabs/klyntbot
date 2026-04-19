//! Test-only helpers for migrations.
#[cfg(test)]
pub async fn run_notifications_migrations(pool: &sqlx::SqlitePool) {
    sqlx::query(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../notifications/migrations/001_notification_tables.sql"
    )))
    .execute(pool)
    .await
    .unwrap();
}
