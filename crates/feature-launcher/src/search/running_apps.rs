use crate::types::*;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

struct RunningApp {
    name: String,
    pid: u32,
    path: PathBuf,
    icon_data: Option<String>,
}

#[derive(Clone)]
pub struct RunningAppsSource {
    apps: Arc<RwLock<Vec<RunningApp>>>,
    icon_cache: Option<Arc<platform_macos::apps::AppIconCache>>,
}

impl RunningAppsSource {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: None,
        }
    }

    pub fn with_icon_cache(icon_cache: Arc<platform_macos::apps::AppIconCache>) -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: Some(icon_cache),
        }
    }

    fn get_running_apps(
        icon_cache: Option<&platform_macos::apps::AppIconCache>,
    ) -> Vec<RunningApp> {
        let tmp_dir = std::env::temp_dir().join("klyntbot-icons-tmp");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let apps = platform_macos::apps::running_applications()
            .into_iter()
            .map(|a| {
                let path = a.path.unwrap_or_default();
                let icon_data = icon_cache.and_then(|cache| {
                    let (icon, _) = cache.resolve_icon(&path, &tmp_dir);
                    icon
                });
                RunningApp {
                    name: a.name,
                    pid: a.pid as u32,
                    path,
                    icon_data,
                }
            })
            .collect();

        let _ = std::fs::remove_dir_all(&tmp_dir);
        apps
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
        let cache = self.icon_cache.clone();
        let apps = tokio::task::spawn_blocking(move || {
            Self::get_running_apps(cache.as_deref())
        })
        .await
        .unwrap_or_default();
        tracing::debug!("Refreshed {} running apps", apps.len());
        *self.apps.write() = apps;
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let apps = self.apps.read();
        let scored = super::fuzzy_match(query, &apps, |app| &app.name, limit);

        scored
            .into_iter()
            .map(|(score, app)| LauncherItem {
                id: format!("running:{}", app.pid),
                title: app.name.clone(),
                subtitle: Some("Running".to_string()),
                icon: app.icon_data.clone().or_else(|| Some("activity".to_string())),
                kind: LauncherItemKind::RunningApp {
                    pid: app.pid,
                    path: app.path.clone(),
                },
                score: (score as f64) / 1000.0 * 1.2,
            })
            .collect()
    }
}
