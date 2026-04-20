use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct PrefPane {
    name: String,
    bundle_id: String,
}

#[derive(Clone)]
pub struct SystemPrefsSource {
    panes: Arc<RwLock<Vec<PrefPane>>>,
}

impl Default for SystemPrefsSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemPrefsSource {
    pub fn new() -> Self {
        Self {
            panes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    #[cfg(target_os = "macos")]
    fn scan_panes() -> Vec<PrefPane> {
        use std::process::Command;

        let dirs = [
            Path::new("/System/Library/PreferencePanes"),
            Path::new("/Library/PreferencePanes"),
        ];
        let home = std::env::var("HOME").unwrap_or_default();
        let user_dir = PathBuf::from(&home).join("Library/PreferencePanes");

        let mut panes = Vec::new();
        for dir in dirs.iter().chain(std::iter::once(&user_dir.as_path())) {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                #[allow(clippy::unnecessary_map_or)]
                if path.extension().map_or(true, |e| e != "prefPane") {
                    continue;
                }
                let plist = path.join("Contents/Info.plist");
                if !plist.exists() {
                    continue;
                }
                let bundle_out = Command::new("/usr/libexec/PlistBuddy")
                    .args(["-c", "Print :CFBundleIdentifier", &plist.to_string_lossy()])
                    .output();
                let bundle_id = match bundle_out {
                    Ok(o) if o.status.success() => {
                        String::from_utf8_lossy(&o.stdout).trim().to_string()
                    }
                    _ => continue,
                };
                let name = Command::new("/usr/libexec/PlistBuddy")
                    .args(["-c", "Print :NSPrefPaneIconLabel", &plist.to_string_lossy()])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .or_else(|| {
                        Command::new("/usr/libexec/PlistBuddy")
                            .args(["-c", "Print :CFBundleName", &plist.to_string_lossy()])
                            .output()
                            .ok()
                            .filter(|o| o.status.success())
                            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    })
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    });

                panes.push(PrefPane { name, bundle_id });
            }
        }
        panes
    }

    #[cfg(not(target_os = "macos"))]
    fn scan_panes() -> Vec<PrefPane> {
        vec![]
    }
}

#[async_trait::async_trait]
impl super::SearchSource for SystemPrefsSource {
    fn name(&self) -> &'static str {
        "system_prefs"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let panes = self.panes.read();
        let scored = super::fuzzy_match(query, &panes, |p| &p.name, limit);

        scored
            .into_iter()
            .map(|(score, p)| LauncherItem {
                id: format!("pref:{}", p.bundle_id),
                title: p.name.clone(),
                subtitle: Some("System Settings".to_string()),
                icon: Some("settings".to_string()),
                kind: LauncherItemKind::SystemPref {
                    pane_id: p.bundle_id.clone(),
                },
                score: (score as f64) / 1000.0 * 0.6,
                no_view: false,
                arguments: vec![],
            })
            .collect()
    }

    async fn refresh(&self) {
        let panes = tokio::task::spawn_blocking(Self::scan_panes)
            .await
            .unwrap_or_default();
        tracing::info!("Indexed {} system preference panes", panes.len());
        *self.panes.write() = panes;
    }
}
