use std::sync::Arc;

use feature_launcher::{
    AppIndex, ClipboardRepo, FrequencyRepo, ScriptRunner, SearchSource, SourceRegistry,
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
    if let Err(e) = StoragePool::run_feature_migrations(
        &pool,
        &feature_launcher::LauncherFeature::migrations_static(),
    )
    .await
    {
        error!("launcher migration failed — feature disabled: {e}");
        return LauncherResult {
            launcher_engine: None,
        };
    }

    let launcher_config = &config.launcher;
    let frequency_repo = FrequencyRepo::new(pool.clone());
    let clipboard_repo = ClipboardRepo::new(pool);

    let mut sources: Vec<Arc<dyn feature_launcher::SearchSource>> = Vec::new();

    // Apps source
    if launcher_config.sources.apps.enabled {
        let icon_cache_dir = config.data_dir_path().join("cache").join("app-icons");
        let app_index = Arc::new(AppIndex::with_cache_dir(icon_cache_dir));
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
    if launcher_config.sources.files.enabled {
        let source = Arc::new(feature_launcher::FileSearchSource::new(
            launcher_config.sources.files.scan_dirs.clone(),
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
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
        sources.push(Arc::new(feature_launcher::RunningAppsSource::new()));
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

    // Browser history — pre-loaded, refreshed by BackgroundRefresher
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
            interval: std::time::Duration::from_secs(3),
        });
    }
    if let Some(s) = find_source("contacts") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(30),
        });
    }
    if let Some(s) = find_source("browser_history") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(120),
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

    // File watches
    let mut watches: Vec<(std::path::PathBuf, Arc<dyn feature_launcher::SearchSource>)> =
        Vec::new();

    if let Some(path) = feature_launcher::BookmarksSource::browser_bookmarks_path(
        &launcher_config.sources.bookmarks.browser,
    ) {
        if let Some(s) = find_source("bookmarks") {
            watches.push((path, s));
        }
    }
    if launcher_config.sources.ssh_hosts.enabled {
        let home = std::env::var("HOME").unwrap_or_default();
        let ssh_config = std::path::PathBuf::from(&home).join(".ssh/config");
        if let Some(s) = find_source("ssh_hosts") {
            watches.push((ssh_config, s));
        }
    }
    if launcher_config.sources.scripts.enabled {
        let scripts_dir = shellexpand::tilde(&launcher_config.sources.scripts.dir).to_string();
        if let Some(s) = find_source("scripts") {
            watches.push((std::path::PathBuf::from(&scripts_dir), s));
        }
    }

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
        registry,
        frequency_repo,
        clipboard_repo,
        _file_watcher: file_watcher,
    });

    info!("launcher feature initialized");
    LauncherResult {
        launcher_engine: Some(engine),
    }
}
