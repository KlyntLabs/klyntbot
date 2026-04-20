use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ScriptEntry {
    pub name: String,
    pub path: PathBuf,
    pub icon: Option<String>,
    pub description: Option<String>,
}

pub struct ScriptMetadata {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone)]
pub struct ScriptRunner {
    scripts: Arc<RwLock<Vec<ScriptEntry>>>,
    scripts_dir: Option<PathBuf>,
}

impl ScriptRunner {
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(RwLock::new(Vec::new())),
            scripts_dir: None,
        }
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        Self {
            scripts: Arc::new(RwLock::new(Vec::new())),
            scripts_dir: Some(dir),
        }
    }

    pub fn set_scripts(&self, scripts: Vec<ScriptEntry>) {
        *self.scripts.write() = scripts;
    }

    pub fn parse_metadata(content: &str) -> ScriptMetadata {
        let mut meta = ScriptMetadata {
            name: None,
            icon: None,
            description: None,
        };
        for line in content.lines().take(5) {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("# name:") {
                meta.name = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("# icon:") {
                meta.icon = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("# description:") {
                meta.description = Some(rest.trim().to_string());
            }
        }
        meta
    }

    pub fn discover(dir: &Path) -> Vec<ScriptEntry> {
        let mut scripts = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return scripts,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());
            if !matches!(ext, Some("sh" | "applescript" | "scpt")) {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let meta = Self::parse_metadata(&content);
            let name = meta.name.unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
            scripts.push(ScriptEntry {
                name,
                path,
                icon: meta.icon,
                description: meta.description,
            });
        }
        scripts
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let scripts = self.scripts.read();
        let scored = super::fuzzy_match2(
            query,
            &scripts,
            |s| &s.name,
            |s| s.description.as_deref(),
            limit,
        );

        scored
            .into_iter()
            .map(|(score, s)| LauncherItem {
                id: format!("script:{}", s.path.display()),
                title: s.name.clone(),
                subtitle: s.description.clone(),
                icon: s.icon.clone().or_else(|| Some("file-code".to_string())),
                kind: LauncherItemKind::Script {
                    path: s.path.clone(),
                    name: s.name.clone(),
                },
                score: (score as f64) / 1000.0 * 0.6,
                no_view: false,
                arguments: vec![],
            })
            .collect()
    }

    pub async fn execute(path: &Path) -> common::Result<String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let output = match ext {
            "applescript" | "scpt" => {
                tokio::process::Command::new("osascript")
                    .arg(path)
                    .output()
                    .await?
            }
            _ => tokio::process::Command::new(path).output().await?,
        };
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(common::KlyntbotError::Io(std::io::Error::other(format!(
                "Script failed: {}",
                stderr
            ))))
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for ScriptRunner {
    fn name(&self) -> &'static str {
        "scripts"
    }

    fn prefix(&self) -> Option<&'static str> {
        Some("/")
    }

    async fn refresh(&self) {
        if let Some(dir) = &self.scripts_dir {
            if dir.exists() {
                let scripts = Self::discover(dir);
                self.set_scripts(scripts);
            }
        }
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        self.search(query, limit)
    }
}

impl Default for ScriptRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_script(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "{}", content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn test_parse_metadata() {
        let content = "#!/bin/bash\n# name: Deploy Staging\n# icon: rocket\n# description: Deploy to staging\necho hello";
        let meta = ScriptRunner::parse_metadata(content);
        assert_eq!(meta.name.as_deref(), Some("Deploy Staging"));
        assert_eq!(meta.icon.as_deref(), Some("rocket"));
        assert_eq!(meta.description.as_deref(), Some("Deploy to staging"));
    }

    #[test]
    fn test_discover_scripts() {
        let dir = tempfile::TempDir::new().unwrap();
        create_script(
            dir.path(),
            "deploy.sh",
            "#!/bin/bash\n# name: Deploy\necho deploy",
        );
        create_script(
            dir.path(),
            "backup.sh",
            "#!/bin/bash\n# name: Backup\necho backup",
        );
        create_script(dir.path(), "readme.txt", "not a script");

        let scripts = ScriptRunner::discover(dir.path());
        assert_eq!(scripts.len(), 2);
    }

    #[test]
    fn test_search_scripts() {
        let runner = ScriptRunner::new();
        runner.set_scripts(vec![
            ScriptEntry {
                name: "Deploy Staging".into(),
                path: "/scripts/deploy.sh".into(),
                icon: None,
                description: None,
            },
            ScriptEntry {
                name: "Backup DB".into(),
                path: "/scripts/backup.sh".into(),
                icon: None,
                description: None,
            },
        ]);
        let results = runner.search("deploy", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Deploy"));
    }
}
