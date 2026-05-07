//! End-to-end test: a single Safari row, decorated by all three sources,
//! with no duplicates. This is the load-bearing regression guard for the
//! decorator pattern — without it, anyone reverting the wiring sees no
//! test failures even though the bug returns.

use feature_launcher::{
    launcher_migrations, new_attention_signals, new_running_signals, AppEntry, AppIndex,
    AttentionSource, EntityAttentionRepo, EntityAttentionRow, LauncherItemKind, RunningSignal,
    SearchSource,
};
use smol_str::SmolStr;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;

async fn fresh_pool() -> sqlx::SqlitePool {
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
async fn safari_appears_once_with_running_and_attention_layers() {
    let pool = fresh_pool().await;

    let running_signals = new_running_signals();
    let attention_signals = new_attention_signals();

    // Build AppIndex with synthetic Safari entry (no plist walk).
    let app_index = AppIndex::new()
        .with_running_signals(Arc::clone(&running_signals))
        .with_attention_signals(Arc::clone(&attention_signals));

    app_index.set_apps(vec![AppEntry {
        name: "Safari".into(),
        path: PathBuf::from("/Applications/Safari.app"),
        bundle_id: Some(SmolStr::new("com.apple.Safari")),
        icon_path: None,
        icon_data_url: None,
    }]);

    // Pre-populate the running signal directly (skip NSWorkspace).
    running_signals.insert(
        SmolStr::new("com.apple.Safari"),
        RunningSignal {
            pid: 99,
            path: PathBuf::from("/Applications/Safari.app"),
        },
    );

    // Seed entity_attention with a Safari record + a competing site.
    let attention_repo = Arc::new(EntityAttentionRepo::new(pool.clone()));
    attention_repo
        .upsert(&EntityAttentionRow {
            canonical_id: "com.apple.Safari".into(),
            kind: "app".into(),
            display_name: "Safari".into(),
            attention_secs: 3600,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: Some("browsing".into()),
        })
        .await
        .unwrap();

    let attention =
        AttentionSource::new(Arc::clone(&attention_repo), Arc::clone(&attention_signals));

    // Drive AttentionSource.search to populate AttentionSignals (this is the
    // "search-time signal push" pattern).
    let _ = attention.search("safari", 10).await;

    // Now query AppIndex — this is the row the user sees.
    let results = app_index.search("safari", 10);
    let safari: Vec<_> = results.iter().filter(|i| i.title == "Safari").collect();

    assert_eq!(
        safari.len(),
        1,
        "expected exactly one Safari row, got {safari:#?}"
    );

    match &safari[0].kind {
        LauncherItemKind::Application { running, .. } => assert!(*running),
        other => panic!("expected Application kind, got {other:?}"),
    }

    let subtitle = safari[0].subtitle.as_deref().unwrap();
    assert!(
        subtitle.contains("Running"),
        "subtitle missing 'Running': {subtitle:?}"
    );
    assert!(
        subtitle.contains("1h"),
        "subtitle missing '1h' (3600s): {subtitle:?}"
    );

    assert_eq!(safari[0].id, "app:com.apple.Safari");
}

#[tokio::test]
async fn attention_only_app_with_no_install_is_suppressed() {
    let pool = fresh_pool().await;

    let attention_signals = new_attention_signals();

    let attention_repo = Arc::new(EntityAttentionRepo::new(pool.clone()));
    attention_repo
        .upsert(&EntityAttentionRow {
            canonical_id: "com.gone.App".into(),
            kind: "app".into(),
            display_name: "Gone App".into(),
            attention_secs: 999,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: None,
        })
        .await
        .unwrap();

    let attention =
        AttentionSource::new(Arc::clone(&attention_repo), Arc::clone(&attention_signals));

    let items = attention.search("gone", 10).await;
    assert!(
        items.is_empty(),
        "uninstalled app must not appear (orphan suppression), got {items:?}"
    );
    // But the signal is still recorded for any installed app to consume:
    assert!(attention_signals.contains_key(&SmolStr::new("com.gone.App")));
}

#[tokio::test]
async fn site_attention_still_emits_url_navigation() {
    let pool = fresh_pool().await;

    let attention_signals = new_attention_signals();

    let attention_repo = Arc::new(EntityAttentionRepo::new(pool.clone()));
    attention_repo
        .upsert(&EntityAttentionRow {
            canonical_id: "github.com".into(),
            kind: "site".into(),
            display_name: "GitHub".into(),
            attention_secs: 3600,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: Some("coding".into()),
        })
        .await
        .unwrap();

    let attention = AttentionSource::new(Arc::clone(&attention_repo), attention_signals);

    let items = attention.search("github", 10).await;
    assert_eq!(items.len(), 1);
    assert!(matches!(
        items[0].kind,
        LauncherItemKind::UrlNavigation { .. }
    ));
}
