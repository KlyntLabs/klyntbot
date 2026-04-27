use crate::types::*;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct BrewEntry {
    name: String,
    is_cask: bool,
}

#[derive(Clone)]
pub struct BrewSource {
    packages: Arc<RwLock<Vec<BrewEntry>>>,
}

impl Default for BrewSource {
    fn default() -> Self {
        Self::new()
    }
}

impl BrewSource {
    pub fn new() -> Self {
        Self {
            packages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn scan_packages() -> Vec<BrewEntry> {
        let mut entries = Vec::new();

        if which::which("brew").is_err() {
            tracing::info!("brew not found — BrewSource disabled");
            return entries;
        }

        // Formulae
        if let Ok(output) = std::process::Command::new("brew")
            .args(["list", "--formula", "-1"])
            .output()
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let name = line.trim();
                    if !name.is_empty() {
                        entries.push(BrewEntry {
                            name: name.to_string(),
                            is_cask: false,
                        });
                    }
                }
            }
        }

        // Casks
        if let Ok(output) = std::process::Command::new("brew")
            .args(["list", "--cask", "-1"])
            .output()
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let name = line.trim();
                    if !name.is_empty() {
                        entries.push(BrewEntry {
                            name: name.to_string(),
                            is_cask: true,
                        });
                    }
                }
            }
        }

        entries
    }
}

#[async_trait::async_trait]
impl super::SearchSource for BrewSource {
    fn name(&self) -> &'static str {
        "brew"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let packages = self.packages.read();
        let scored = super::fuzzy_match(query, &packages, |p| &p.name, limit);

        scored
            .into_iter()
            .map(|(score, p)| {
                let kind_label = if p.is_cask { "Cask" } else { "Formula" };
                LauncherItem {
                    id: format!("brew:{}", p.name),
                    title: p.name.clone(),
                    subtitle: Some(format!("Homebrew {kind_label}")),
                    icon: Some("package".to_string()),
                    kind: LauncherItemKind::BrewPackage {
                        name: p.name.clone(),
                        is_cask: p.is_cask,
                    },
                    score: (score as f64) / 1000.0 * 0.4,
                    no_view: false,
                    arguments: vec![],
                                    pinned: false,
                    }
            })
            .collect()
    }

    async fn refresh(&self) {
        let packages = tokio::task::spawn_blocking(Self::scan_packages)
            .await
            .unwrap_or_default();
        tracing::info!("Indexed {} brew packages", packages.len());
        *self.packages.write() = packages;
    }
}
