use std::collections::HashMap;

use chrono::Utc;
use desktop_shared::errors::ApiError;
use feature_launcher::{
    Calculator, ClipboardRepo, FrequencyRepo, LauncherItem, LauncherItemKind, SourceRegistry,
    UrlNavigation, WindowManager,
};
use feature_notes::repo::NoteRepo;
use storage::Repos;
use tokio_util::sync::CancellationToken;

use crate::errors::map_storage_err;

/// Central search engine that fans out queries to all providers.
pub struct LauncherSearchEngine {
    pub(crate) registry: SourceRegistry,
    pub(crate) frequency_repo: FrequencyRepo,
    pub(crate) clipboard_repo: ClipboardRepo,
    /// Held so the OS file-watcher thread is joined on drop.
    pub(crate) _file_watcher: Option<feature_launcher::SourceFileWatcher>,
    /// Held so the clipboard monitor task is cancelled on drop.
    pub(crate) _clipboard_cancel: Option<CancellationToken>,
    pub(crate) window_manager: WindowManager,
}

impl LauncherSearchEngine {
    /// Main search entry point with prefix routing.
    pub async fn search(
        &self,
        query: &str,
        repos: &Repos,
        note_repo: &NoteRepo,
    ) -> Result<Vec<LauncherItem>, ApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }

        // Calculator handles both prefix (=) and universal
        let calc_results = Calculator::try_eval(query)
            .map(|r| {
                vec![LauncherItem {
                    id: format!("calc:{}", r.expression),
                    title: format!("{}", r.result),
                    subtitle: Some(r.expression.clone()),
                    icon: Some("calculator".to_string()),
                    kind: LauncherItemKind::Calculator {
                        expression: r.expression,
                        result: r.result,
                    },
                    score: 2.0,
                }]
            })
            .unwrap_or_default();

        // Registry handles prefix routing + fan-out
        let mut results = self.registry.search(query, 10).await;

        // Add DB-backed sources (tasks, notes) — these aren't in registry
        // because they need external repos
        let (tasks, notes) = tokio::join!(
            self.search_tasks(query, repos),
            self.search_notes(query, note_repo),
        );
        results.extend(tasks.unwrap_or_default());
        results.extend(notes.unwrap_or_default());

        // Add calculator results
        results.extend(calc_results);

        // URL navigation — detect URL-like queries and offer to open in browser
        if let Some(url_item) = UrlNavigation::try_match(query) {
            results.push(url_item);
        }

        // Apply frequency boosts
        self.apply_frequency_boosts(&mut results).await;

        // Deduplicate by canonical key (URL or path), keeping the higher-scored item
        results = Self::deduplicate(results);

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(20);

        // Always add AI chat fallback at the end
        results.push(LauncherItem {
            id: format!("ai:{}", query),
            title: format!("Ask AI: {}", query),
            subtitle: Some("Chat with your AI assistant".to_string()),
            icon: Some("message-circle".to_string()),
            kind: LauncherItemKind::AiChat {
                query: query.to_string(),
            },
            score: 0.0,
        });

        Ok(results)
    }

    async fn search_tasks(
        &self,
        query: &str,
        repos: &Repos,
    ) -> Result<Vec<LauncherItem>, ApiError> {
        let tasks = repos
            .tasks
            .search_by_keyword(query, Some(5))
            .await
            .map_err(map_storage_err)?;

        Ok(tasks
            .into_iter()
            .map(|t| LauncherItem {
                id: format!("task:{}", t.id),
                title: t.title.clone(),
                subtitle: t.description.clone(),
                icon: Some("check-square".to_string()),
                kind: LauncherItemKind::Task {
                    task_id: t.id.clone(),
                    status: t.status.clone(),
                },
                score: if t.status == "doing" { 0.9 } else { 0.7 },
            })
            .collect())
    }

    async fn search_notes(
        &self,
        query: &str,
        note_repo: &NoteRepo,
    ) -> Result<Vec<LauncherItem>, ApiError> {
        let notes = note_repo
            .search_notes(query)
            .await
            .map_err(map_storage_err)?;

        Ok(notes
            .into_iter()
            .take(5)
            .map(|n| {
                let preview: String = n.body.chars().take(100).collect();
                LauncherItem {
                    id: format!("note:{}", n.id),
                    title: n.title.clone(),
                    subtitle: Some(preview.clone()),
                    icon: Some("file-text".to_string()),
                    kind: LauncherItemKind::Note {
                        note_id: n.id.clone(),
                        preview,
                    },
                    score: 0.6,
                }
            })
            .collect())
    }

    async fn apply_frequency_boosts(&self, items: &mut [LauncherItem]) {
        // Collect (item_id, kind) pairs for boostable items
        let pairs: Vec<(usize, String, String)> = items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let kind_str = match &item.kind {
                    LauncherItemKind::Application { .. } => "app",
                    LauncherItemKind::Task { .. } => "task",
                    LauncherItemKind::Note { .. } => "note",
                    LauncherItemKind::ClipboardEntry { .. } => "clip",
                    LauncherItemKind::SystemCommand { .. } => "system",
                    LauncherItemKind::Script { .. } => "script",
                    LauncherItemKind::Calculator { .. }
                    | LauncherItemKind::AiChat { .. }
                    | LauncherItemKind::UrlNavigation { .. } => return None,
                    LauncherItemKind::Calendar { .. } => "calendar",
                    LauncherItemKind::File { .. } => "file",
                    LauncherItemKind::ContentMatch { .. } => "grep",
                    LauncherItemKind::Contact { .. } => "contact",
                    LauncherItemKind::SystemPref { .. } => "pref",
                    LauncherItemKind::RunningApp { .. } => "running_app",
                    LauncherItemKind::Bookmark { .. } => "bookmark",
                    LauncherItemKind::BrowserHistory { .. } => "history",
                    LauncherItemKind::BrewPackage { .. } => "brew",
                    LauncherItemKind::SshHost { .. } => "ssh",
                    LauncherItemKind::GitRepo { .. } => "repo",
                };
                Some((i, item.id.clone(), kind_str.to_string()))
            })
            .collect();

        if pairs.is_empty() {
            return;
        }

        let batch_keys: Vec<(String, String)> = pairs
            .iter()
            .map(|(_, id, k)| (id.clone(), k.clone()))
            .collect();

        if let Ok(boosts) = self.frequency_repo.get_boosts_batch(&batch_keys).await {
            let now = Utc::now();
            for ((idx, _, _), (boost, last_used)) in pairs.iter().zip(boosts.iter()) {
                let recency_multiplier = last_used
                    .as_ref()
                    .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
                    .map(|dt| {
                        let age = now.signed_duration_since(dt);
                        if age.num_hours() < 1 {
                            2.0
                        } else if age.num_hours() < 24 {
                            1.5
                        } else if age.num_days() < 7 {
                            1.0
                        } else {
                            0.5
                        }
                    })
                    .unwrap_or(1.0);
                items[*idx].score += boost * 0.1 * recency_multiplier;
            }
        }
    }

    /// Extract a canonical key for deduplication.
    /// Items that represent the same underlying resource share a key.
    fn canonical_key(item: &LauncherItem) -> String {
        match &item.kind {
            LauncherItemKind::Application { path, .. }
            | LauncherItemKind::RunningApp { path, .. } => {
                format!("path:{}", path.to_string_lossy().to_lowercase())
            }
            LauncherItemKind::Bookmark { url, .. }
            | LauncherItemKind::BrowserHistory { url, .. }
            | LauncherItemKind::UrlNavigation { url } => {
                // Normalize URL: strip trailing slash and lowercase
                let normalized = url.trim_end_matches('/').to_lowercase();
                format!("url:{}", normalized)
            }
            _ => format!("id:{}", item.id),
        }
    }

    /// Deduplicate items by canonical key, keeping the higher-scored item.
    fn deduplicate(items: Vec<LauncherItem>) -> Vec<LauncherItem> {
        let mut best: HashMap<String, LauncherItem> = HashMap::new();
        for item in items {
            let key = Self::canonical_key(&item);
            match best.get(&key) {
                Some(existing) if existing.score >= item.score => {}
                _ => {
                    best.insert(key, item);
                }
            }
        }
        best.into_values().collect()
    }

    /// Record that an item was executed (for frequency boosting).
    pub async fn record_execution(&self, item_id: &str, kind: &str) -> Result<(), ApiError> {
        self.frequency_repo
            .increment(item_id, kind)
            .await
            .map_err(map_storage_err)
    }
}
