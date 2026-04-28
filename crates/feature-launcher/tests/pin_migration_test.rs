//! Verifies the one-shot rewrite of pin + frequency IDs from path-based
//! to bundle-id-based after AppIndex first identifies bundle IDs.

use feature_launcher::{launcher_migrations, migrate_app_ids_to_bundle_ids, AppEntry};
use smol_str::SmolStr;
use sqlx::Row;
use std::path::PathBuf;
use storage::StoragePool;

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = StoragePool::connect_in_memory()
        .await
        .unwrap()
        .inner()
        .clone();
    StoragePool::run_feature_migrations(&pool, &launcher_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn migrates_pins_and_frequency_to_bundle_ids() {
    let pool = setup_pool().await;

    // Seed: pin Safari by path (pre-migration shape).
    sqlx::query("INSERT INTO launcher_pins (item_id, kind, position) VALUES (?1, ?2, ?3)")
        .bind("app:/Applications/Safari.app")
        .bind("application")
        .bind(0)
        .execute(&pool)
        .await
        .unwrap();

    // Seed: usage log entry by path.
    sqlx::query("INSERT INTO launcher_usage_log (item_id, kind, used_at) VALUES (?1, ?2, ?3)")
        .bind("app:/Applications/Safari.app")
        .bind("application")
        .bind("2026-04-28T12:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

    let apps = vec![AppEntry {
        name: "Safari".into(),
        path: PathBuf::from("/Applications/Safari.app"),
        bundle_id: Some(SmolStr::new("com.apple.Safari")),
        icon_path: None,
    }];

    let migrated = migrate_app_ids_to_bundle_ids(&pool, &apps).await.unwrap();
    assert_eq!(migrated, 2, "expected 2 rows updated (1 pin + 1 usage)");

    // Verify pin rewritten.
    let row = sqlx::query("SELECT item_id FROM launcher_pins WHERE kind = 'application'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let item_id: String = row.get("item_id");
    assert_eq!(item_id, "app:com.apple.Safari");

    // Verify usage log rewritten.
    let row = sqlx::query("SELECT item_id FROM launcher_usage_log WHERE kind = 'application'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let item_id: String = row.get("item_id");
    assert_eq!(item_id, "app:com.apple.Safari");
}

#[tokio::test]
async fn idempotent_when_already_migrated() {
    let pool = setup_pool().await;

    sqlx::query("INSERT INTO launcher_pins (item_id, kind, position) VALUES (?1, ?2, ?3)")
        .bind("app:com.apple.Safari") // already migrated
        .bind("application")
        .bind(0)
        .execute(&pool)
        .await
        .unwrap();

    let apps = vec![AppEntry {
        name: "Safari".into(),
        path: PathBuf::from("/Applications/Safari.app"),
        bundle_id: Some(SmolStr::new("com.apple.Safari")),
        icon_path: None,
    }];

    let migrated = migrate_app_ids_to_bundle_ids(&pool, &apps).await.unwrap();
    assert_eq!(migrated, 0, "no rows to migrate (already done)");
}

#[tokio::test]
async fn skips_apps_without_bundle_id() {
    let pool = setup_pool().await;

    sqlx::query("INSERT INTO launcher_pins (item_id, kind, position) VALUES (?1, ?2, ?3)")
        .bind("app:/Applications/Weird.app")
        .bind("application")
        .bind(0)
        .execute(&pool)
        .await
        .unwrap();

    let apps = vec![AppEntry {
        name: "Weird".into(),
        path: PathBuf::from("/Applications/Weird.app"),
        bundle_id: None,
        icon_path: None,
    }];

    let migrated = migrate_app_ids_to_bundle_ids(&pool, &apps).await.unwrap();
    assert_eq!(migrated, 0, "apps without bundle_id are not migrated");

    // The pin row is unchanged — still path-keyed (correct fallback).
    let row = sqlx::query("SELECT item_id FROM launcher_pins WHERE kind = 'application'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let item_id: String = row.get("item_id");
    assert_eq!(item_id, "app:/Applications/Weird.app");
}
