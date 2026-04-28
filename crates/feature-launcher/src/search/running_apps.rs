use crate::types::*;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

struct RunningApp {
    name: String,
    pid: u32,
    path: PathBuf,
}

#[derive(Clone)]
pub struct RunningAppsSource {
    apps: Arc<RwLock<Vec<RunningApp>>>,
    icon_cache_dir: Option<PathBuf>,
}

impl RunningAppsSource {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache_dir: None,
        }
    }

    pub fn with_icon_cache_dir(dir: PathBuf) -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache_dir: Some(dir),
        }
    }
}

impl Default for RunningAppsSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl super::SearchSource for RunningAppsSource {
    fn name(&self) -> &'static str {
        "running_apps"
    }

    async fn refresh(&self) {
        // Only collect app metadata — NO icon resolution here.
        // Icon resolution triggers macOS IconServices mmap that leaks ~1GB over time.
        let apps = tokio::task::spawn_blocking(|| {
            platform_macos::apps::running_applications()
                .into_iter()
                .map(|a| RunningApp {
                    name: a.name,
                    pid: a.pid as u32,
                    path: a.path.unwrap_or_default(),
                })
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        tracing::debug!("Refreshed {} running apps (icons deferred)", apps.len());
        *self.apps.write() = apps;
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let apps = self.apps.read();
        let scored = super::fuzzy_match(query, &apps, |app| &app.name, limit);

        scored
            .into_iter()
            .map(|(score, app)| {
                let icon = self
                    .icon_cache_dir
                    .as_ref()
                    .and_then(|dir| {
                        let stem = app.path.file_stem()?.to_string_lossy().replace(' ', "_");
                        let png = dir.join(format!("{stem}.png"));
                        // If not cached yet (e.g. Finder in CoreServices), extract via sips
                        if !png.exists() {
                            let icon_cache = platform_macos::apps::AppIconCache::new(dir.clone());
                            icon_cache.resolve_icon_path(&app.path);
                        }
                        let bytes = std::fs::read(&png).ok()?;
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        Some(format!("data:image/png;base64,{b64}"))
                    })
                    .or_else(|| Some("running-app".to_string()));

                LauncherItem {
                    id: format!("running:{}", app.pid),
                    title: app.name.clone(),
                    subtitle: Some("Running".to_string()),
                    icon,
                    kind: LauncherItemKind::RunningApp {
                        pid: app.pid,
                        path: app.path.clone(),
                    },
                    score: (score as f64) / 1000.0 * 1.2,
                    no_view: false,
                    arguments: vec![],
                                    pinned: false,
                    }
            })
            .collect()
    }
}
