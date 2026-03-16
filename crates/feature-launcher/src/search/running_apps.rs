use crate::types::*;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone)]
pub struct RunningAppsSource {
    apps: Arc<RwLock<Vec<(String, u32, std::path::PathBuf)>>>,
}

impl Default for RunningAppsSource {
    fn default() -> Self {
        Self::new()
    }
}

impl RunningAppsSource {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn get_running_apps() -> Vec<(String, u32, std::path::PathBuf)> {
        platform_macos::apps::running_applications()
            .into_iter()
            .map(|a| (a.name, a.pid as u32, a.path.unwrap_or_default()))
            .collect()
    }
}

#[async_trait::async_trait]
impl super::SearchSource for RunningAppsSource {
    fn name(&self) -> &'static str {
        "running_apps"
    }

    async fn refresh(&self) {
        let apps = tokio::task::spawn_blocking(Self::get_running_apps)
            .await
            .unwrap_or_default();
        tracing::debug!("Refreshed {} running apps", apps.len());
        *self.apps.write() = apps;
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let apps = self.apps.read();
        let scored = super::fuzzy_match(query, &apps, |app| &app.0, limit);

        scored
            .into_iter()
            .map(|(score, app)| LauncherItem {
                id: format!("running:{}", app.1),
                title: app.0.clone(),
                subtitle: Some("Running".to_string()),
                icon: Some("activity".to_string()),
                kind: LauncherItemKind::RunningApp {
                    pid: app.1,
                    path: app.2.clone(),
                },
                score: (score as f64) / 1000.0 * 1.2,
            })
            .collect()
    }
}
