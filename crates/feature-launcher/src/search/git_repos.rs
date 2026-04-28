use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct RepoEntry {
    name: String,
    path: PathBuf,
}

#[derive(Clone)]
pub struct GitReposSource {
    repos: Arc<RwLock<Vec<RepoEntry>>>,
    scan_dirs: Vec<String>,
}

impl GitReposSource {
    pub fn new(scan_dirs: Vec<String>) -> Self {
        Self {
            repos: Arc::new(RwLock::new(Vec::new())),
            scan_dirs,
        }
    }

    fn scan_repos(dirs: &[String], max_depth: usize) -> Vec<RepoEntry> {
        let mut repos = Vec::new();
        for dir in dirs {
            let expanded = shellexpand::tilde(dir).to_string();
            let path = Path::new(&expanded);
            if path.exists() {
                Self::walk_for_repos(path, max_depth, &mut repos);
            }
        }
        repos
    }

    fn walk_for_repos(dir: &Path, depth: usize, repos: &mut Vec<RepoEntry>) {
        if depth == 0 {
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.join(".git").exists() {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                repos.push(RepoEntry { name, path });
                continue;
            }
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if dir_name.starts_with('.') || dir_name == "node_modules" || dir_name == "target" {
                continue;
            }
            Self::walk_for_repos(&path, depth - 1, repos);
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for GitReposSource {
    fn name(&self) -> &'static str {
        "git_repos"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let repos = self.repos.read();
        let scored = super::fuzzy_match(query, &repos, |r| &r.name, limit);

        scored
            .into_iter()
            .map(|(score, r)| LauncherItem {
                id: format!("repo:{}", r.path.display()),
                title: r.name.clone(),
                subtitle: Some(r.path.display().to_string()),
                icon: Some("git-branch".to_string()),
                kind: LauncherItemKind::GitRepo {
                    path: r.path.clone(),
                },
                score: (score as f64) / 1000.0 * 0.8,
                no_view: false,
                arguments: vec![],
                pinned: false,
            })
            .collect()
    }

    async fn refresh(&self) {
        let repos = Self::scan_repos(&self.scan_dirs, 3);
        tracing::info!("Indexed {} git repos", repos.len());
        *self.repos.write() = repos;
    }
}
