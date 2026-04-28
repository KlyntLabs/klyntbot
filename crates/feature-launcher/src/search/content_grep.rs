use crate::types::*;
use std::path::PathBuf;

pub struct ContentGrepSource {
    default_scope: String,
    rg_available: bool,
}

impl ContentGrepSource {
    pub fn new(default_scope: String) -> Self {
        let rg_available = which::which("rg").is_ok();
        if !rg_available {
            tracing::info!("rg not found — content grep disabled");
        }
        Self {
            default_scope,
            rg_available,
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for ContentGrepSource {
    fn name(&self) -> &'static str {
        "content_grep"
    }

    fn prefix(&self) -> Option<&'static str> {
        Some("g/")
    }

    fn cache_ttl(&self) -> Option<std::time::Duration> {
        // rg spawn dominates miss cost (~50ms); longer TTL amortizes better
        Some(std::time::Duration::from_secs(8))
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        if !self.rg_available {
            return vec![];
        }

        let scope = shellexpand::tilde(&self.default_scope).to_string();

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio::process::Command::new("rg")
                .args(["--json", "-m", "1", "--max-count", "1", query, &scope])
                .output(),
        )
        .await;

        let output = match output {
            Ok(Ok(o)) => o,
            _ => return vec![],
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        let mut seen_files = std::collections::HashSet::new();

        for line in stdout.lines() {
            if results.len() >= limit {
                break;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if json.get("type").and_then(|t| t.as_str()) != Some("match") {
                continue;
            }
            let Some(data) = json.get("data") else {
                continue;
            };
            let Some(path_str) = data
                .get("path")
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
            else {
                continue;
            };
            if !seen_files.insert(path_str.to_string()) {
                continue;
            }
            let line_num = data
                .get("line_number")
                .and_then(|n| n.as_u64())
                .unwrap_or(0) as u32;
            let preview = data
                .get("lines")
                .and_then(|l| l.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim()
                .chars()
                .take(100)
                .collect::<String>();
            let path = PathBuf::from(path_str);
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            results.push(LauncherItem {
                id: format!("grep:{}:{}", path.display(), line_num),
                title: file_name,
                subtitle: Some(preview.clone()),
                icon: Some("search".to_string()),
                kind: LauncherItemKind::ContentMatch {
                    path,
                    line: line_num,
                    preview,
                },
                score: 0.7,
                no_view: false,
                arguments: vec![],
                pinned: false,
            });
        }

        results
    }
}
