use std::str::FromStr;
use std::sync::Arc;

use smol_str::SmolStr;

use crate::repos::{EntityAttentionRepo, EntityAttentionRow};
use crate::search::signals::{AttentionSignals, AttentionStat};
use crate::search::SearchSource;
use crate::types::{LauncherItem, LauncherItemKind};

pub struct AttentionSource {
    repo: Arc<EntityAttentionRepo>,
    signals: AttentionSignals,
}

impl AttentionSource {
    pub fn new(repo: Arc<EntityAttentionRepo>, signals: AttentionSignals) -> Self {
        Self { repo, signals }
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
            Ok(rows) => route_rows_to_items_and_signals(rows, &self.signals),
            Err(e) => {
                tracing::warn!(error = %e, "AttentionSource search failed");
                vec![]
            }
        }
    }
}

/// Pure routing function so we can test without a database.
/// - `kind = "site"` rows become `UrlNavigation` items (unchanged behavior).
/// - `kind = "app"` rows are pushed into `signals` and **not emitted** —
///   AppIndex owns the unified Application row.
/// - Other kinds are dropped.
pub(crate) fn route_rows_to_items_and_signals(
    rows: Vec<EntityAttentionRow>,
    signals: &AttentionSignals,
) -> Vec<LauncherItem> {
    rows.into_iter()
        .filter_map(|row| match row.kind.as_str() {
            "site" => Some(into_site_item(row)),
            "app" => {
                if let Ok(ts) = jiff::Timestamp::from_str(&row.last_used_at) {
                    signals.insert(
                        SmolStr::new(&row.canonical_id),
                        AttentionStat {
                            attention_secs: row.attention_secs,
                            category: row.category.map(SmolStr::new),
                            last_used_at: ts,
                        },
                    );
                } else {
                    tracing::warn!(
                        "AttentionSource: skipping app row with bad timestamp: {}",
                        row.last_used_at
                    );
                }
                None
            }
            _ => None,
        })
        .collect()
}

fn into_site_item(row: EntityAttentionRow) -> LauncherItem {
    LauncherItem {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_site_item_handles_https_prefix() {
        let row = EntityAttentionRow {
            canonical_id: "https://github.com".into(),
            kind: "site".into(),
            display_name: "GitHub".into(),
            attention_secs: 100,
            last_used_at: "2026-04-28T12:00:00Z".into(),
            icon_hint: None,
            category: None,
        };
        let item = into_site_item(row);
        match item.kind {
            LauncherItemKind::UrlNavigation { url } => assert_eq!(url, "https://github.com"),
            _ => panic!("expected UrlNavigation"),
        }
    }

    #[test]
    fn route_app_rows_to_signals_emits_only_sites() {
        use crate::search::signals::new_attention_signals;
        use smol_str::SmolStr;

        let signals = new_attention_signals();
        let rows = vec![
            EntityAttentionRow {
                canonical_id: "com.apple.Safari".into(),
                kind: "app".into(),
                display_name: "Safari".into(),
                attention_secs: 3600,
                last_used_at: "2026-04-28T12:00:00Z".into(),
                icon_hint: None,
                category: Some("browsing".into()),
            },
            EntityAttentionRow {
                canonical_id: "github.com".into(),
                kind: "site".into(),
                display_name: "GitHub".into(),
                attention_secs: 7200,
                last_used_at: "2026-04-28T13:00:00Z".into(),
                icon_hint: None,
                category: Some("coding".into()),
            },
        ];

        let items = route_rows_to_items_and_signals(rows, &signals);

        assert_eq!(items.len(), 1, "only the site row becomes an item");
        assert_eq!(items[0].title, "GitHub");

        assert_eq!(signals.len(), 1, "the app row went into signals");
        let safari = signals.get(&SmolStr::new("com.apple.Safari")).unwrap();
        assert_eq!(safari.attention_secs, 3600);
        assert_eq!(safari.category.as_deref(), Some("browsing"));
    }

    #[test]
    fn site_rows_unchanged_in_format() {
        use crate::search::signals::new_attention_signals;
        let signals = new_attention_signals();
        let rows = vec![EntityAttentionRow {
            canonical_id: "github.com".into(),
            kind: "site".into(),
            display_name: "GitHub".into(),
            attention_secs: 100,
            last_used_at: "2026-04-28T12:00:00Z".into(),
            icon_hint: None,
            category: None,
        }];
        let items = route_rows_to_items_and_signals(rows, &signals);
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            LauncherItemKind::UrlNavigation { url } => assert_eq!(url, "https://github.com"),
            other => panic!("expected UrlNavigation, got {other:?}"),
        }
    }
}
