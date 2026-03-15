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
    /// Directory for persistent icon cache (e.g., `{data_dir}/cache/app-icons/`)
    cache_dir: Option<PathBuf>,
}

impl AppIndex {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            cache_dir: None,
        }
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&cache_dir);
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            cache_dir: Some(cache_dir),
        }
    }

    pub fn set_apps(&self, apps: Vec<AppEntry>) {
        *self.apps.write() = apps;
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let apps = self.apps.read();
        let scored = super::fuzzy_match(query, &apps, |app| &app.name, limit);

        scored
            .into_iter()
            .map(|(score, app)| LauncherItem {
                id: format!("app:{}", app.path.display()),
                title: app.name.clone(),
                subtitle: Some(app.path.display().to_string()),
                icon: app
                    .icon_data
                    .clone()
                    .or_else(|| Some("app-window".to_string())),
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

        // Extract icons — use disk cache when available
        let mut cache_hits = 0usize;
        let tmp_dir = std::env::temp_dir().join("klyntbot-icons-tmp");
        let _ = std::fs::create_dir_all(&tmp_dir);

        for app in &mut apps {
            let (icon, hit) = self.resolve_icon(&app.path, &tmp_dir);
            app.icon_data = icon;
            if hit {
                cache_hits += 1;
            }
        }

        let _ = std::fs::remove_dir_all(&tmp_dir);

        tracing::info!(
            "Indexed {} applications ({} icon cache hits)",
            apps.len(),
            cache_hits,
        );
        self.set_apps(apps);
    }

    /// Resolve an app icon, returning (data_uri, was_cache_hit).
    /// Checks disk cache first; falls back to sips extraction.
    #[cfg(target_os = "macos")]
    fn resolve_icon(&self, app_path: &Path, tmp_dir: &Path) -> (Option<String>, bool) {
        let stem = match app_path.file_stem() {
            Some(s) => s.to_string_lossy().replace(' ', "_"),
            None => return (None, false),
        };

        let app_mtime = Self::get_mtime(app_path).unwrap_or(0);

        // Try disk cache
        if let Some(ref cache_dir) = self.cache_dir {
            let cached_png = cache_dir.join(format!("{stem}.png"));
            let cached_mtime = cache_dir.join(format!("{stem}.mtime"));

            if cached_png.exists() && cached_mtime.exists() {
                if let Ok(stored) = std::fs::read_to_string(&cached_mtime) {
                    if stored.trim().parse::<u64>().ok() == Some(app_mtime) {
                        // Cache hit — read PNG and encode
                        if let Ok(png_bytes) = std::fs::read(&cached_png) {
                            use base64::Engine;
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                            return (Some(format!("data:image/png;base64,{b64}")), true);
                        }
                    }
                }
            }
        }

        // Cache miss — extract via sips
        let data_uri = Self::extract_icon(app_path, tmp_dir);

        // Write to disk cache for next time
        if let (Some(ref cache_dir), Some(ref uri)) = (&self.cache_dir, &data_uri) {
            // Decode the base64 back to PNG bytes for disk storage
            if let Some(b64_data) = uri.strip_prefix("data:image/png;base64,") {
                use base64::Engine;
                if let Ok(png_bytes) = base64::engine::general_purpose::STANDARD.decode(b64_data) {
                    let _ = std::fs::write(cache_dir.join(format!("{stem}.png")), &png_bytes);
                    let _ = std::fs::write(
                        cache_dir.join(format!("{stem}.mtime")),
                        app_mtime.to_string(),
                    );
                }
            }
        }

        (data_uri, false)
    }

    #[cfg(target_os = "macos")]
    fn get_mtime(path: &Path) -> Option<u64> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta.modified().ok()?;
        Some(mtime.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
    }

    /// Extract an app's icon as a base64 data URI using sips.
    #[cfg(target_os = "macos")]
    fn extract_icon(app_path: &Path, tmp_dir: &Path) -> Option<String> {
        use std::process::Command;

        let plist_path = app_path.join("Contents/Info.plist");
        let output = Command::new("/usr/libexec/PlistBuddy")
            .args([
                "-c",
                "Print :CFBundleIconFile",
                &plist_path.to_string_lossy(),
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let mut icon_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !icon_name.ends_with(".icns") {
            icon_name.push_str(".icns");
        }

        let icns_path = app_path.join("Contents/Resources").join(&icon_name);
        if !icns_path.exists() {
            return None;
        }

        let stem = app_path.file_stem()?.to_string_lossy().replace(' ', "_");
        let png_path = tmp_dir.join(format!("{stem}.png"));

        let sips_result = Command::new("sips")
            .args([
                "-s",
                "format",
                "png",
                "--resampleWidth",
                "32",
                &icns_path.to_string_lossy(),
                "--out",
                &png_path.to_string_lossy(),
            ])
            .output()
            .ok()?;

        if !sips_result.status.success() {
            return None;
        }

        let png_bytes = std::fs::read(&png_path).ok()?;
        let _ = std::fs::remove_file(&png_path);

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        Some(format!("data:image/png;base64,{b64}"))
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
