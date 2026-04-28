use std::collections::HashMap;
use std::sync::Arc;

use desktop_shared::errors::ApiError;
use feature_launcher::{
    Calculator, ClipboardRepo, FileKind, FrequencyRepo, LauncherItem, LauncherItemKind, PinsRepo,
    SourceRegistry, UrlNavigation,
};
use feature_notes::repo::NoteRepo;
use storage::Repos;

use crate::errors::map_storage_err;

/// Central search engine that fans out queries to all providers.
pub struct LauncherSearchEngine {
    pub registry: Arc<SourceRegistry>,
    pub frequency_repo: Arc<FrequencyRepo>,
    pub clipboard_repo: Arc<ClipboardRepo>,
    pub pins_repo: Arc<PinsRepo>,
    /// Stored here so the OS watcher thread is joined on drop.
    pub _file_watcher: Option<feature_launcher::SourceFileWatcher>,
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
            let top = self
                .frequency_repo
                .top_frecency(8)
                .await
                .unwrap_or_default();
            let mut defaults = Vec::with_capacity(top.len());

            for (item_id, kind, score) in &top {
                let item = match kind.as_str() {
                    "app" | "running_app" => {
                        let path_str = item_id
                            .strip_prefix("app:")
                            .or_else(|| item_id.strip_prefix("running:"))
                            .unwrap_or(item_id.as_str());
                        let path = std::path::Path::new(path_str);
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if name.is_empty() {
                            continue;
                        }
                        let results = self.registry.search(&name, 1).await;
                        results.into_iter().next().map(|mut item| {
                            item.score = *score;
                            item
                        })
                    }
                    "file" | "grep" => {
                        let path_str = item_id
                            .strip_prefix("file:")
                            .or_else(|| item_id.strip_prefix("grep:"))
                            .unwrap_or(item_id.as_str());
                        let path = std::path::Path::new(path_str);
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        Some(LauncherItem {
                            id: item_id.clone(),
                            title: name,
                            subtitle: Some(path_str.to_string()),
                            icon: Some("file".to_string()),
                            kind: LauncherItemKind::File {
                                path: path.to_path_buf(),
                                kind: FileKind::File,
                            },
                            score: *score,
                            no_view: false,
                            arguments: vec![],
                            pinned: false,
                        })
                    }
                    _ => None,
                };
                if let Some(item) = item {
                    defaults.push(item);
                }
            }

            return Ok(defaults);
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
                    no_view: false,
                    arguments: vec![],
                    pinned: false,
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

        // Apply pin elevation
        self.apply_pin_elevation(&mut results).await;

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
            no_view: false,
            arguments: vec![],
            pinned: false,
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
                no_view: false,
                arguments: vec![],
                pinned: false,
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
                    no_view: false,
                    arguments: vec![],
                    pinned: false,
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
                    LauncherItemKind::WindowAction { .. } => "window",
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
            for ((idx, _, _), (boost, _last_used)) in pairs.iter().zip(boosts.iter()) {
                // Frecency is already time-weighted — just add it as a boost
                items[*idx].score += boost * 0.3;
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

    async fn apply_pin_elevation(&self, items: &mut [LauncherItem]) {
        let pinned = match self.pins_repo.pinned_set().await {
            Ok(set) => set,
            Err(_) => return,
        };

        for item in items {
            let tag = kind_tag(&item.kind);
            if pinned.contains(&(item.id.clone(), tag.to_string())) {
                item.pinned = true;
                item.score += 1000.0;
            }
        }
    }

    /// Record that an item was executed (for frequency boosting).
    pub async fn record_execution(&self, item_id: &str, kind: &str) -> Result<(), ApiError> {
        self.frequency_repo
            .increment(item_id, kind)
            .await
            .map_err(map_storage_err)
    }

    /// Execute a launcher item: record execution and return a result envelope.
    pub async fn execute(
        &self,
        item_id: &str,
        kind: &str,
    ) -> Result<feature_launcher::LauncherExecuteResult, ApiError> {
        self.record_execution(item_id, kind).await?;
        Ok(feature_launcher::LauncherExecuteResult::default())
    }
}

fn kind_tag(kind: &feature_launcher::LauncherItemKind) -> &'static str {
    use feature_launcher::LauncherItemKind as K;
    match kind {
        K::Application { .. } => "application",
        K::Task { .. } => "task",
        K::Note { .. } => "note",
        K::ClipboardEntry { .. } => "clipboardEntry",
        K::SystemCommand { .. } => "systemCommand",
        K::Script { .. } => "script",
        K::Calculator { .. } => "calculator",
        K::Calendar { .. } => "calendar",
        K::AiChat { .. } => "aiChat",
        K::File { .. } => "file",
        K::ContentMatch { .. } => "contentMatch",
        K::Contact { .. } => "contact",
        K::SystemPref { .. } => "systemPref",
        K::RunningApp { .. } => "runningApp",
        K::Bookmark { .. } => "bookmark",
        K::BrowserHistory { .. } => "browserHistory",
        K::BrewPackage { .. } => "brewPackage",
        K::SshHost { .. } => "sshHost",
        K::GitRepo { .. } => "gitRepo",
        K::UrlNavigation { .. } => "urlNavigation",
        K::WindowAction { .. } => "windowAction",
    }
}
