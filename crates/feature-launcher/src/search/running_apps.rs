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

#[cfg(target_os = "macos")]
impl RunningAppsSource {
    fn get_running_apps() -> Vec<(String, u32, std::path::PathBuf)> {
        // Use sysctl/ps approach instead of objc2 to avoid heavy dependencies
        let output = match std::process::Command::new("osascript")
            .args([
                "-l",
                "JavaScript",
                "-e",
                r#"
                var apps = Application("System Events").processes.whose({backgroundOnly: false})();
                var results = [];
                for (var i = 0; i < apps.length; i++) {
                    try {
                        results.push({
                            name: apps[i].name(),
                            pid: apps[i].unixId(),
                            path: apps[i].file() ? apps[i].file().posixPath() : ""
                        });
                    } catch(e) {}
                }
                JSON.stringify(results);
                "#,
            ])
            .output()
        {
            Ok(o) if o.status.success() => o,
            _ => return vec![],
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let apps: Vec<serde_json::Value> = serde_json::from_str(stdout.trim()).unwrap_or_default();

        apps.into_iter()
            .filter_map(|a| {
                let name = a.get("name")?.as_str()?.to_string();
                let pid = a.get("pid")?.as_u64()? as u32;
                let path_str = a.get("path").and_then(|p| p.as_str()).unwrap_or("");
                Some((name, pid, std::path::PathBuf::from(path_str)))
            })
            .collect()
    }
}

#[cfg(not(target_os = "macos"))]
impl RunningAppsSource {
    fn get_running_apps() -> Vec<(String, u32, std::path::PathBuf)> {
        vec![]
    }
}
