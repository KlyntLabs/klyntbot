use crate::search::signals::{
    new_attention_signals, new_running_signals, AttentionSignals, RunningSignals,
};
use crate::types::*;
use parking_lot::RwLock;
use smol_str::SmolStr;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<SmolStr>,
    /// Path to the cached 64x64 PNG icon (resolved via sips).
    pub icon_path: Option<PathBuf>,
}

impl AppEntry {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        if ext != "app" {
            return None;
        }
        let name = path.file_stem()?.to_string_lossy().to_string();
        Some(Self {
            name,
            path: path.to_path_buf(),
            bundle_id: None,
            icon_path: None,
        })
    }
}

#[derive(Clone)]
pub struct AppIndex {
    apps: Arc<RwLock<Vec<AppEntry>>>,
    /// Shared icon cache backed by `platform_macos::apps::AppIconCache`.
    icon_cache: Option<Arc<platform_macos::apps::AppIconCache>>,
    /// Bundle-ID-keyed live "is running" map, refreshed by RunningAppsSource.
    running_signals: RunningSignals,
    /// Bundle-ID-keyed cumulative attention stats, written by AttentionSource.
    attention_signals: AttentionSignals,
}

impl AppIndex {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: None,
            running_signals: new_running_signals(),
            attention_signals: new_attention_signals(),
        }
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: Some(Arc::new(platform_macos::apps::AppIconCache::new(cache_dir))),
            running_signals: new_running_signals(),
            attention_signals: new_attention_signals(),
        }
    }

    /// Builder: attach an externally-owned RunningSignals map (so other sources
    /// can write into it). Without this call, `AppIndex` owns a private empty map.
    pub fn with_running_signals(mut self, signals: RunningSignals) -> Self {
        self.running_signals = signals;
        self
    }

    /// Builder: attach an externally-owned AttentionSignals map.
    pub fn with_attention_signals(mut self, signals: AttentionSignals) -> Self {
        self.attention_signals = signals;
        self
    }

    pub fn icon_cache(&self) -> Option<Arc<platform_macos::apps::AppIconCache>> {
        self.icon_cache.clone()
    }

    pub fn running_signals(&self) -> RunningSignals {
        Arc::clone(&self.running_signals)
    }

    pub fn attention_signals(&self) -> AttentionSignals {
        Arc::clone(&self.attention_signals)
    }

    #[cfg(test)]
    pub fn running_signals_for_test(&self) -> RunningSignals {
        Arc::clone(&self.running_signals)
    }

    #[cfg(test)]
    pub fn attention_signals_for_test(&self) -> AttentionSignals {
        Arc::clone(&self.attention_signals)
    }

    pub fn set_apps(&self, apps: Vec<AppEntry>) {
        *self.apps.write() = apps;
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let apps = self.apps.read();
        let scored = super::fuzzy_match(query, &apps, |app| &app.name, limit);

        scored
            .into_iter()
            .map(|(score, app)| {
                // Convert cached PNG to base64 data URI at search time.
                // Icons are 64x64 (~5KB each) — tiny compared to the old 1024x1024.
                let icon = app
                    .icon_path
                    .as_ref()
                    .and_then(|p| {
                        let bytes = std::fs::read(p).ok()?;
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        Some(format!("data:image/png;base64,{b64}"))
                    })
                    .or_else(|| Some("app-window".to_string()));

                LauncherItem {
                    id: format!("app:{}", app.path.display()),
                    title: app.name.clone(),
                    subtitle: Some(app.path.display().to_string()),
                    icon,
                    kind: LauncherItemKind::Application {
                        path: app.path.clone(),
                        running: false,
                    },
                    score: (score as f64) / 1000.0,
                    no_view: false,
                    arguments: vec![],
                    pinned: false,
                }
            })
            .collect()
    }

    #[cfg(target_os = "macos")]
    pub async fn index_applications(&self) {
        let dirs = [
            "/Applications",
            "/System/Applications",
            "/System/Library/CoreServices",
            "/System/Library/CoreServices/Applications",
        ];
        let home = std::env::var("HOME").unwrap_or_default();
        let user_apps = format!("{}/Applications", home);

        let mut apps = Vec::new();
        for dir in dirs.iter().chain(std::iter::once(&user_apps.as_str())) {
            // CoreServices nests one level deeper than /Applications.
            let max_depth = if dir.starts_with("/System/Library/CoreServices") {
                4
            } else {
                3
            };
            if let Ok(entries) = Self::walk_apps(Path::new(dir), max_depth) {
                apps.extend(entries);
            }
        }

        // Populate bundle_id from Info.plist for every app.
        for app in &mut apps {
            app.bundle_id = platform_macos::apps::read_bundle_id(&app.path).map(SmolStr::new);
        }

        // Dedupe by bundle_id, preferring /Applications over system locations.
        apps = Self::dedupe_by_bundle_id(apps);

        if let Some(cache) = &self.icon_cache {
            for app in &mut apps {
                app.icon_path = cache.resolve_icon_path(&app.path);
            }
        }
        let icon_count = apps.iter().filter(|a| a.icon_path.is_some()).count();
        let bundle_count = apps.iter().filter(|a| a.bundle_id.is_some()).count();
        tracing::info!(
            "Indexed {} applications ({} with icons, {} with bundle ids)",
            apps.len(),
            icon_count,
            bundle_count
        );
        self.set_apps(apps);
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn walk_apps(dir: &Path, max_depth: usize) -> std::io::Result<Vec<AppEntry>> {
        let mut apps = Vec::new();
        if max_depth == 0 {
            return Ok(apps);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "app") {
                if let Some(app) = AppEntry::from_path(&path) {
                    apps.push(app);
                }
            } else if path.is_dir() {
                if let Ok(sub) = Self::walk_apps(&path, max_depth - 1) {
                    apps.extend(sub);
                }
            }
        }
        Ok(apps)
    }

    /// Dedupe app entries by bundle_id, preferring `/Applications` over system / nested paths.
    /// Apps without a bundle_id are kept as-is (their identity is path-based).
    pub(crate) fn dedupe_by_bundle_id(mut apps: Vec<AppEntry>) -> Vec<AppEntry> {
        apps.sort_by_key(|a| match a.path.starts_with("/Applications") {
            true => 0,
            false => 1,
        });
        let mut seen: HashSet<SmolStr> = HashSet::new();
        apps.retain(|a| match &a.bundle_id {
            Some(b) => seen.insert(b.clone()),
            None => true,
        });
        apps
    }

    #[cfg(not(target_os = "macos"))]
    pub async fn index_applications(&self) {
        // No-op on non-macOS
    }
}

impl Default for AppIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl super::SearchSource for AppIndex {
    fn name(&self) -> &'static str {
        "apps"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        self.search(query, limit)
    }

    async fn refresh(&self) {
        self.index_applications().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_app_entry_from_path() {
        let path = PathBuf::from("/Applications/Safari.app");
        let entry = AppEntry::from_path(&path);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "Safari");
    }

    #[test]
    fn test_fuzzy_search() {
        let index = AppIndex::new();
        index.set_apps(vec![
            AppEntry {
                name: "Visual Studio Code".into(),
                path: "/Applications/Visual Studio Code.app".into(),
                bundle_id: None,
                icon_path: None,
            },
            AppEntry {
                name: "Safari".into(),
                path: "/Applications/Safari.app".into(),
                bundle_id: None,
                icon_path: None,
            },
            AppEntry {
                name: "Slack".into(),
                path: "/Applications/Slack.app".into(),
                bundle_id: None,
                icon_path: None,
            },
        ]);
        let results = index.search("vsc", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Visual Studio Code"));
    }

    #[test]
    fn test_search_empty_returns_none() {
        let index = AppIndex::new();
        index.set_apps(vec![AppEntry {
            name: "Safari".into(),
            path: "/Applications/Safari.app".into(),
            bundle_id: None,
            icon_path: None,
        }]);
        let results = index.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn index_populates_bundle_id_from_plist() {
        use std::fs;
        let dir = tempfile::TempDir::new().unwrap();
        let app_path = dir.path().join("Foo.app");
        let contents = app_path.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(
            contents.join("Info.plist"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <plist version=\"1.0\"><dict>\n\
             <key>CFBundleIdentifier</key><string>com.example.Foo</string>\n\
             </dict></plist>\n",
        ).unwrap();

        let apps = AppIndex::walk_apps(dir.path(), 3).unwrap();
        let apps_with_bid: Vec<_> = apps.iter().filter_map(|a| {
            let bid = platform_macos::apps::read_bundle_id(&a.path)?;
            Some((a.name.clone(), bid))
        }).collect();
        assert_eq!(apps_with_bid, vec![("Foo".to_string(), "com.example.Foo".to_string())]);
    }

    #[test]
    fn dedupe_by_bundle_id_keeps_user_path() {
        let apps = vec![
            AppEntry {
                name: "Safari".into(),
                path: "/System/Applications/Safari.app".into(),
                bundle_id: Some("com.apple.Safari".into()),
                icon_path: None,
            },
            AppEntry {
                name: "Safari".into(),
                path: "/Applications/Safari.app".into(),
                bundle_id: Some("com.apple.Safari".into()),
                icon_path: None,
            },
        ];
        let deduped = AppIndex::dedupe_by_bundle_id(apps);
        assert_eq!(deduped.len(), 1);
        assert_eq!(
            deduped[0].path,
            std::path::PathBuf::from("/Applications/Safari.app")
        );
    }

    #[test]
    fn dedupe_keeps_path_keyed_entries() {
        let apps = vec![
            AppEntry {
                name: "Foo".into(),
                path: "/Applications/Foo.app".into(),
                bundle_id: None,
                icon_path: None,
            },
            AppEntry {
                name: "Bar".into(),
                path: "/Applications/Bar.app".into(),
                bundle_id: None,
                icon_path: None,
            },
        ];
        let deduped = AppIndex::dedupe_by_bundle_id(apps);
        assert_eq!(deduped.len(), 2, "path-keyed entries cannot dupe by bundle");
    }

    #[test]
    fn with_signals_attaches_maps() {
        use crate::search::signals::{new_attention_signals, new_running_signals};
        let running = new_running_signals();
        let attention = new_attention_signals();
        let idx = AppIndex::new()
            .with_running_signals(running.clone())
            .with_attention_signals(attention.clone());

        assert!(Arc::ptr_eq(&running, &idx.running_signals_for_test()));
        assert!(Arc::ptr_eq(&attention, &idx.attention_signals_for_test()));
    }
}
