use crate::types::*;
use parking_lot::RwLock;
use std::collections::HashMap;
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
    icon_cache: Option<Arc<platform_macos::apps::AppIconCache>>,
    /// In-memory cache of resolved icon data URIs keyed by app path.
    /// Prevents repeated NSWorkspace calls which cause macOS IconServices
    /// to mmap gigabytes of .isdata files over the process lifetime.
    resolved_icons: Arc<RwLock<HashMap<PathBuf, Option<String>>>>,
}

impl RunningAppsSource {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: None,
            resolved_icons: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_icon_cache(icon_cache: Arc<platform_macos::apps::AppIconCache>) -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: Some(icon_cache),
            resolved_icons: Arc::new(RwLock::new(HashMap::new())),
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
                // Resolve icon lazily with in-memory cache — only for search results.
                let icon = {
                    let cache = self.resolved_icons.read();
                    if let Some(cached) = cache.get(&app.path) {
                        cached.clone()
                    } else {
                        drop(cache);
                        let resolved = self.icon_cache.as_ref().and_then(|c| {
                            let tmp = std::env::temp_dir().join("klyntbot-icons-tmp");
                            let _ = std::fs::create_dir_all(&tmp);
                            let (icon, _) = c.resolve_icon(&app.path, &tmp);
                            icon
                        });
                        self.resolved_icons
                            .write()
                            .insert(app.path.clone(), resolved.clone());
                        resolved
                    }
                }
                .or_else(|| Some("activity".to_string()));

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
                }
            })
            .collect()
    }
}
