use std::sync::Arc;

use feature_launcher::{
    AppIndex, AttentionSource, ClipboardRepo, EntityAttentionRepo, FileSearchSource, FrequencyRepo,
    FsEventKind, ScriptRunner, SearchSource, SourceRegistry,
};
use storage::StoragePool;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::handlers::launcher::LauncherSearchEngine;

/// Results from launcher initialization phase.
pub(super) struct LauncherResult {
    pub launcher_engine: Option<Arc<LauncherSearchEngine>>,
}

/// Initialize the launcher feature (always enabled).
pub(super) async fn init_launcher(
    config: &config::Config,
    storage_pool: &StoragePool,
    shutdown_token: &CancellationToken,
) -> LauncherResult {
    let pool = storage_pool.inner().clone();

    // Run feature migrations
    if let Err(e) =
        StoragePool::run_feature_migrations(&pool, &feature_launcher::launcher_migrations()).await
    {
        error!("launcher migration failed — feature disabled: {e}");
        return LauncherResult {
            launcher_engine: None,
        };
    }

    let launcher_config = &config.launcher;
    let frequency_repo = FrequencyRepo::new(pool.clone());
    let clipboard_repo = ClipboardRepo::new(pool.clone());
    let pins_repo = feature_launcher::PinsRepo::new(&storage_pool);
    let entity_attention_repo = Arc::new(EntityAttentionRepo::new(pool.clone()));

    let mut sources: Vec<Arc<dyn feature_launcher::SearchSource>> = Vec::new();
    let icon_cache_dir = config.data_dir_path().join("cache").join("app-icons");

    // Apps source
    if launcher_config.sources.apps.enabled {
        let app_index = Arc::new(AppIndex::with_cache_dir(icon_cache_dir.clone()));
        let idx = Arc::clone(&app_index);
        tokio::spawn(async move { idx.index_applications().await });
        sources.push(app_index);
    }

    // Scripts source (with_dir enables refresh/re-discovery)
    if launcher_config.sources.scripts.enabled {
        let scripts_dir = shellexpand::tilde(&launcher_config.sources.scripts.dir).to_string();
        let scripts_path = std::path::PathBuf::from(&scripts_dir);
        let script_runner = Arc::new(ScriptRunner::with_dir(scripts_path.clone()));
        if scripts_path.exists() {
            let scripts = ScriptRunner::discover(&scripts_path);
            info!("discovered {} launcher scripts", scripts.len());
            script_runner.set_scripts(scripts);
        }
        sources.push(script_runner);
    }

    // System commands (always enabled — lightweight)
    sources.push(Arc::new(feature_launcher::SystemCommands));

    // Window presets — prefix w/
    if launcher_config.sources.window_presets.enabled {
        sources.push(Arc::new(feature_launcher::WindowPresetsSource));
    }

    // Clipboard source
    if launcher_config.sources.clipboard.enabled {
        sources.push(Arc::new(clipboard_repo.clone()));
    }

    // System preferences
    if launcher_config.sources.system_prefs.enabled {
        let source = Arc::new(feature_launcher::SystemPrefsSource::new());
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // Brew packages
    if launcher_config.sources.brew.enabled {
        let source = Arc::new(feature_launcher::BrewSource::new());
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // SSH hosts
    if launcher_config.sources.ssh_hosts.enabled {
        let source = Arc::new(feature_launcher::SshHostsSource::new());
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // Git repos
    if launcher_config.sources.git_repos.enabled {
        let source = Arc::new(feature_launcher::GitReposSource::new(
            launcher_config.sources.git_repos.scan_dirs.clone(),
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // File search (ignore-walk index) — pre-indexed, refreshed by BackgroundRefresher
    // Keep a typed clone so we can wire incremental FSEvent updates later.
    let file_search_source: Option<Arc<FileSearchSource>>;
    if launcher_config.sources.files.enabled {
        let source = Arc::new(feature_launcher::FileSearchSource::new(
            launcher_config.sources.files.scan_dirs.clone(),
            launcher_config.sources.files.max_entries,
            launcher_config.sources.files.mdfind_fallback,
            launcher_config.sources.files.mdfind_threshold,
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        file_search_source = Some(Arc::clone(&source));
        let file_cfg = &launcher_config.sources.files;
        let interval = std::time::Duration::from_secs(file_cfg.rebuild_interval_min * 60);
        let fs_for_rebuild = Arc::clone(&source);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            tick.tick().await; // skip immediate first tick (initial build already runs)
            loop {
                tick.tick().await;
                tracing::info!("launcher: triggering 30-min safety rebuild");
                fs_for_rebuild.refresh().await;
            }
        });
        sources.push(source);
    } else {
        file_search_source = None;
    }

    // Content grep (rg) — prefix ?, live query, cached by SourceRegistry
    if launcher_config.sources.content_grep.enabled {
        sources.push(Arc::new(feature_launcher::ContentGrepSource::new(
            launcher_config.sources.content_grep.default_scope.clone(),
        )));
    }

    // Contacts — prefix @, pre-loaded index refreshed by BackgroundRefresher
    if launcher_config.sources.contacts.enabled {
        sources.push(Arc::new(feature_launcher::ContactsSource::new()));
    }

    // Running apps — pre-loaded index refreshed by BackgroundRefresher
    if launcher_config.sources.running_apps.enabled {
        let source = Arc::new(feature_launcher::RunningAppsSource::with_icon_cache_dir(
            icon_cache_dir.clone(),
        ));
        sources.push(source);
    }

    // Browser bookmarks — pre-loaded, refreshed by SourceFileWatcher
    if launcher_config.sources.bookmarks.enabled {
        let source = Arc::new(feature_launcher::BookmarksSource::new(
            launcher_config.sources.bookmarks.browser.clone(),
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // Browser history — pre-loaded, refreshed by SourceFileWatcher (FSEvents)
    if launcher_config.sources.browser_history.enabled {
        let source = Arc::new(feature_launcher::BrowserHistorySource::new(
            launcher_config.sources.browser_history.browser.clone(),
            launcher_config.sources.browser_history.max_days,
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // ── Build refresh entries and file watches BEFORE consuming sources ──

    let find_source = |name: &str| -> Option<Arc<dyn feature_launcher::SearchSource>> {
        sources.iter().find(|s| s.name() == name).cloned()
    };

    let mut refresh_entries: Vec<feature_launcher::RefreshEntry> = Vec::new();

    if let Some(s) = find_source("running_apps") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(10),
        });
    }
    if let Some(s) = find_source("contacts") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(30),
        });
    }
    if let Some(s) = find_source("brew") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(300),
        });
    }
    if let Some(s) = find_source("git_repos") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(300),
        });
    }
    if let Some(s) = find_source("files") {
        let interval_secs = launcher_config.sources.files.refresh_interval_secs;
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(interval_secs),
        });
    }
    if let Some(s) = find_source("apps") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(300),
        });
    }

    // File watches (FSEvents via notify)
    let mut watches: Vec<feature_launcher::WatchEntry> = Vec::new();

    if let Some(path) = feature_launcher::BookmarksSource::browser_bookmarks_path(
        &launcher_config.sources.bookmarks.browser,
    ) {
        if let Some(s) = find_source("bookmarks") {
            watches.push(feature_launcher::WatchEntry {
                path,
                source: s,
                min_interval: None,
                recursive: false,
                on_change: None,
            });
        }
    }
    if let Some(path) = feature_launcher::BrowserHistorySource::history_db_path(
        &launcher_config.sources.browser_history.browser,
    ) {
        if let Some(s) = find_source("browser_history") {
            watches.push(feature_launcher::WatchEntry {
                path,
                source: s,
                min_interval: Some(std::time::Duration::from_secs(10)),
                recursive: false,
                on_change: None,
            });
        }
    }
    if launcher_config.sources.ssh_hosts.enabled {
        let home = std::env::var("HOME").unwrap_or_default();
        let ssh_config = std::path::PathBuf::from(&home).join(".ssh/config");
        if let Some(s) = find_source("ssh_hosts") {
            watches.push(feature_launcher::WatchEntry {
                path: ssh_config,
                source: s,
                min_interval: None,
                recursive: false,
                on_change: None,
            });
        }
    }
    if launcher_config.sources.git_repos.enabled {
        if let Some(s) = find_source("git_repos") {
            for dir_str in &launcher_config.sources.git_repos.scan_dirs {
                let dir = std::path::PathBuf::from(shellexpand::tilde(dir_str).to_string());
                if dir.exists() {
                    watches.push(feature_launcher::WatchEntry {
                        path: dir,
                        source: Arc::clone(&s),
                        min_interval: Some(std::time::Duration::from_secs(30)),
                        recursive: true,
                        on_change: None,
                    });
                }
            }
        }
    }
    if launcher_config.sources.scripts.enabled {
        let scripts_dir = shellexpand::tilde(&launcher_config.sources.scripts.dir).to_string();
        if let Some(s) = find_source("scripts") {
            watches.push(feature_launcher::WatchEntry {
                path: std::path::PathBuf::from(&scripts_dir),
                source: s,
                min_interval: None,
                recursive: false,
                on_change: None,
            });
        }
    }
    // File search incremental updates — attach a custom FSEvent handler per scan dir
    // that calls `apply_events` instead of the full `refresh()`.
    if let (Some(file_search), Some(s)) = (file_search_source.as_ref(), find_source("files")) {
        for dir_str in &launcher_config.sources.files.scan_dirs {
            let dir = std::path::PathBuf::from(shellexpand::tilde(dir_str).to_string());
            if dir.exists() {
                let fs = Arc::clone(file_search);
                watches.push(feature_launcher::WatchEntry {
                    path: dir,
                    source: Arc::clone(&s),
                    min_interval: None,
                    recursive: true,
                    on_change: Some(Arc::new(move |events| {
                        let fs = Arc::clone(&fs);
                        Box::pin(async move {
                            let changes: Vec<_> = events
                                .into_iter()
                                .map(|ev| {
                                    let kind = if ev.path.exists() {
                                        FsEventKind::Create
                                    } else {
                                        FsEventKind::Remove
                                    };
                                    (ev.path, kind)
                                })
                                .collect();
                            fs.apply_events(changes).await;
                        })
                    })),
                });
            }
        }
    }
    if launcher_config.sources.apps.enabled {
        if let Some(s) = find_source("apps") {
            let home = std::env::var("HOME").unwrap_or_default();
            let app_dirs = [
                std::path::PathBuf::from("/Applications"),
                std::path::PathBuf::from("/System/Applications"),
                std::path::PathBuf::from(&home).join("Applications"),
            ];
            for dir in app_dirs {
                if dir.exists() {
                    watches.push(feature_launcher::WatchEntry {
                        path: dir,
                        source: Arc::clone(&s),
                        min_interval: Some(std::time::Duration::from_secs(5)),
                        recursive: true,
                        on_change: None,
                    });
                }
            }
        }
    }

    // Attention source (queries entity_attention table populated by nightly aggregator)
    sources.push(Arc::new(AttentionSource::new(Arc::clone(&entity_attention_repo))));

    // ── Create registry and spawn background services ──

    let registry = SourceRegistry::new(sources);
    let refresh_count = refresh_entries.len();

    if !refresh_entries.is_empty() {
        let query_cache = registry.query_cache();
        let refresher = feature_launcher::BackgroundRefresher::new(
            refresh_entries,
            query_cache,
            shutdown_token.clone(),
        );
        refresher.spawn();
        info!("BackgroundRefresher started with {refresh_count} sources");
    }

    let file_watcher = if !watches.is_empty() {
        match feature_launcher::SourceFileWatcher::start(watches) {
            Ok(watcher) => {
                info!("SourceFileWatcher started");
                Some(watcher)
            }
            Err(e) => {
                tracing::warn!("Failed to start file watcher: {e}");
                None
            }
        }
    } else {
        None
    };

    let engine = Arc::new(LauncherSearchEngine {
        registry: Arc::new(registry),
        frequency_repo: Arc::new(frequency_repo),
        clipboard_repo: Arc::new(clipboard_repo),
        pins_repo: Arc::new(pins_repo),
        entity_attention_repo: Some(Arc::clone(&entity_attention_repo)),
        _file_watcher: file_watcher,
    });

    info!("launcher feature initialized");
    LauncherResult {
        launcher_engine: Some(engine),
    }
}
