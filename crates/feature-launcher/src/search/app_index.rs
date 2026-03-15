use crate::types::*;
use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher,
};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<String>,
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
        })
    }
}

#[derive(Clone)]
pub struct AppIndex {
    apps: Arc<RwLock<Vec<AppEntry>>>,
}

impl AppIndex {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_apps(&self, apps: Vec<AppEntry>) {
        *self.apps.write() = apps;
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        let apps = self.apps.read();
        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &AppEntry)> = apps
            .iter()
            .filter_map(|app| {
                let mut buf = Vec::new();
                let score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&app.name, &mut buf),
                    &mut matcher,
                )?;
                Some((score, app))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, app)| LauncherItem {
                id: format!("app:{}", app.path.display()),
                title: app.name.clone(),
                subtitle: Some(app.path.display().to_string()),
                icon: Some("app-window".to_string()),
                kind: LauncherItemKind::Application {
                    path: app.path.clone(),
                    running: false,
                },
                score: (score as f64) / 1000.0,
            })
            .collect()
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

        tracing::info!("Indexed {} applications", apps.len());
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
            if path.extension().map_or(false, |e| e == "app") {
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
            },
            AppEntry {
                name: "Safari".into(),
                path: "/Applications/Safari.app".into(),
                bundle_id: None,
            },
            AppEntry {
                name: "Slack".into(),
                path: "/Applications/Slack.app".into(),
                bundle_id: None,
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
        }]);
        let results = index.search("", 10);
        assert!(results.is_empty());
    }
}
