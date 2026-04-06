use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<String>,
    /// Base64-encoded PNG icon data URI (e.g., "data:image/png;base64,...")
    pub icon_data: Option<String>,
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
            icon_data: None,
        })
    }
}

#[derive(Clone)]
pub struct AppIndex {
    apps: Arc<RwLock<Vec<AppEntry>>>,
    /// Shared icon cache backed by `platform_macos::apps::AppIconCache`.
    icon_cache: Option<Arc<platform_macos::apps::AppIconCache>>,
}

impl AppIndex {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: None,
        }
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: Some(Arc::new(platform_macos::apps::AppIconCache::new(cache_dir))),
        }
    }

    pub fn icon_cache(&self) -> Option<Arc<platform_macos::apps::AppIconCache>> {
        self.icon_cache.clone()
    }

    pub fn set_apps(&self, apps: Vec<AppEntry>) {
        *self.apps.write() = apps;
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let apps = self.apps.read();
        let scored = super::fuzzy_match(query, &apps, |app| &app.name, limit);

        // Resolve icons lazily — only for the few results returned (typically ≤10).
        // This avoids eagerly mmap'ing macOS IconServices for all 100+ apps.
        let tmp_dir = std::env::temp_dir().join("klyntbot-icons-tmp");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let results: Vec<_> = scored
            .into_iter()
            .map(|(score, app)| {
                let icon = app
                    .icon_data
                    .clone()
                    .or_else(|| {
                        self.icon_cache
                            .as_ref()
                            .and_then(|c| c.resolve_icon(&app.path, &tmp_dir).0)
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
                }
            })
            .collect();

        let _ = std::fs::remove_dir_all(&tmp_dir);
        results
    }

    #[cfg(target_os = "macos")]
    pub async fn index_applications(&self) {
        let dirs = ["/Applications", "/System/Applications"];
        let home = std::env::var("HOME").unwrap_or_default();
        let user_apps = format!("{}/Applications", home);

        let mut apps = Vec::new();
        for dir in dirs.iter().chain(std::iter::once(&user_apps.as_str())) {
            if let Ok(entries) = Self::walk_apps(Path::new(dir), 3) {
                apps.extend(entries);
            }
        }

        // Icons are resolved lazily during search() — NOT at index time.
        // Resolving all 100+ app icons eagerly causes macOS IconServices to
        // mmap ~4GB of icon cache files into the process.
        tracing::info!(
            "Indexed {} applications (icons deferred to search)",
            apps.len()
        );
        self.set_apps(apps);
    }

    #[cfg(target_os = "macos")]
    fn walk_apps(dir: &Path, max_depth: usize) -> std::io::Result<Vec<AppEntry>> {
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
                icon_data: None,
            },
            AppEntry {
                name: "Safari".into(),
                path: "/Applications/Safari.app".into(),
                bundle_id: None,
                icon_data: None,
            },
            AppEntry {
                name: "Slack".into(),
                path: "/Applications/Slack.app".into(),
                bundle_id: None,
                icon_data: None,
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
            icon_data: None,
        }]);
        let results = index.search("", 10);
        assert!(results.is_empty());
    }
}
