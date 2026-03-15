use desktop_shared::errors::ApiError;
use feature_launcher::{
    AppIndex, Calculator, ClipboardContentType, ClipboardRepo, FrequencyRepo, LauncherItem,
    LauncherItemKind, ScriptRunner, SystemCommands,
};
use feature_notes::repo::NoteRepo;
use storage::Repos;

use crate::errors::map_storage_err;

/// Central search engine that fans out queries to all providers.
pub struct LauncherSearchEngine {
    pub app_index: AppIndex,
    pub script_runner: ScriptRunner,
    pub frequency_repo: FrequencyRepo,
    pub clipboard_repo: ClipboardRepo,
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

        // Prefix routing
        if query.starts_with('=') {
            return Ok(self.search_calculator(query));
        }
        if let Some(rest) = query.strip_prefix('>') {
            return Ok(SystemCommands::search(rest.trim()));
        }
        if let Some(rest) = query.strip_prefix('/') {
            return Ok(self.script_runner.search(rest.trim(), 20));
        }

        // Universal search: fan out to all providers
        let (apps, tasks, notes, clipboard, calc) = tokio::join!(
            self.search_apps(query),
            self.search_tasks(query, repos),
            self.search_notes(query, note_repo),
            self.search_clipboard(query),
            async { self.search_calculator(query) },
        );

        let mut results: Vec<LauncherItem> = Vec::new();
        results.extend(apps);
        results.extend(tasks.unwrap_or_default());
        results.extend(notes.unwrap_or_default());
        results.extend(clipboard.unwrap_or_default());
        results.extend(calc);

        // Add system commands if they match
        let system = SystemCommands::search(query);
        results.extend(system);

        // Apply frequency boosts
        self.apply_frequency_boosts(&mut results).await;

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

    async fn search_apps(&self, query: &str) -> Vec<LauncherItem> {
        self.app_index.search(query, 10)
    }

    fn search_calculator(&self, query: &str) -> Vec<LauncherItem> {
        match Calculator::try_eval(query) {
            Some(result) => vec![LauncherItem {
                id: format!("calc:{}", result.expression),
                title: format!("{}", result.result),
                subtitle: Some(result.expression.clone()),
                icon: Some("calculator".to_string()),
                kind: LauncherItemKind::Calculator {
                    expression: result.expression,
                    result: result.result,
                },
                score: 2.0, // Calculator results rank highest when matched
            }],
            None => vec![],
        }
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

    async fn search_clipboard(&self, query: &str) -> Result<Vec<LauncherItem>, ApiError> {
        let entries = self
            .clipboard_repo
            .search(query, 5)
            .await
            .map_err(map_storage_err)?;

        Ok(entries
            .into_iter()
            .map(|e| {
                let content_type = match e.content_type.as_str() {
                    "image" => ClipboardContentType::Image,
                    "file" => ClipboardContentType::File,
                    _ => ClipboardContentType::Text,
                };
                let preview: String = e
                    .preview
                    .clone()
                    .unwrap_or_else(|| e.content.chars().take(80).collect());
                LauncherItem {
                    id: format!("clip:{}", e.id),
                    title: preview,
                    subtitle: e.source_app.clone(),
                    icon: Some("clipboard".to_string()),
                    kind: LauncherItemKind::ClipboardEntry {
                        entry_id: e.id,
                        content_type,
                    },
                    score: 0.5,
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
                    LauncherItemKind::Calculator { .. } => return None,
                    LauncherItemKind::Calendar { .. } => "calendar",
                    LauncherItemKind::AiChat { .. } => return None,
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
            for ((idx, _, _), boost) in pairs.iter().zip(boosts.iter()) {
                items[*idx].score += boost * 0.1;
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
}
