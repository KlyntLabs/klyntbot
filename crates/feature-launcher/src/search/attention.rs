use std::sync::Arc;

use crate::repos::{EntityAttentionRepo, EntityAttentionRow};
use crate::search::SearchSource;
use crate::types::{LauncherItem, LauncherItemKind};

pub struct AttentionSource {
    repo: Arc<EntityAttentionRepo>,
}

impl AttentionSource {
    pub fn new(repo: Arc<EntityAttentionRepo>) -> Self {
        Self { repo }
    }
}

#[async_trait::async_trait]
impl SearchSource for AttentionSource {
    fn name(&self) -> &'static str {
        "attention"
    }

    fn cache_ttl(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(30))
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let rows = if query.is_empty() {
            self.repo.top_by_attention(None, limit as i64).await
        } else {
            self.repo.fts_search(query, limit as i64).await
        };

        match rows {
            Ok(rows) => rows.into_iter().map(into_launcher_item).collect(),
            Err(e) => {
                tracing::warn!(error = %e, "AttentionSource search failed");
                vec![]
            }
        }
    }
}

/// Convert an `EntityAttentionRow` into a `LauncherItem`.
pub fn into_launcher_item(row: EntityAttentionRow) -> LauncherItem {
    match row.kind.as_str() {
        "site" => LauncherItem {
            id: format!("attention:site:{}", row.canonical_id),
            title: row.display_name.clone(),
            subtitle: Some(row.canonical_id.clone()),
            icon: row.icon_hint.or_else(|| Some("globe".to_string())),
            kind: LauncherItemKind::UrlNavigation {
                url: if row.canonical_id.starts_with("http") {
                    row.canonical_id
                } else {
                    format!("https://{}", row.canonical_id)
                },
            },
            score: row.attention_secs as f64,
            no_view: false,
            arguments: vec![],
            pinned: false,
        },
        _ => LauncherItem {
            id: format!("attention:app:{}", row.canonical_id),
            title: row.display_name.clone(),
            subtitle: row.category.clone(),
            icon: row.icon_hint.or_else(|| Some("app".to_string())),
            kind: LauncherItemKind::Application {
                path: std::path::PathBuf::from(&row.canonical_id),
                running: false,
            },
            score: row.attention_secs as f64,
            no_view: false,
            arguments: vec![],
            pinned: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_launcher_item_maps_site_to_url_navigation() {
        let row = EntityAttentionRow {
            canonical_id: "github.com".to_string(),
            kind: "site".to_string(),
            display_name: "GitHub".to_string(),
            attention_secs: 3600,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: Some("coding".to_string()),
        };
        let item = into_launcher_item(row);
        assert_eq!(item.title, "GitHub");
        assert!(
            matches!(item.kind, LauncherItemKind::UrlNavigation { url } if url == "https://github.com")
        );
    }

    #[test]
    fn into_launcher_item_maps_app_to_application() {
        let row = EntityAttentionRow {
            canonical_id: "com.apple.Safari".to_string(),
            kind: "app".to_string(),
            display_name: "Safari".to_string(),
            attention_secs: 1800,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: Some("communication".to_string()),
        };
        let item = into_launcher_item(row);
        assert_eq!(item.title, "Safari");
        assert!(matches!(item.kind, LauncherItemKind::Application { .. }));
    }
}
