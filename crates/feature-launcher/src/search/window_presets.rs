use crate::search::SearchSource;
use crate::types::{LauncherItem, LauncherItemKind, WindowAction};
use crate::window_mgmt::presets::PRESETS;
use async_trait::async_trait;

pub struct WindowPresetsSource;

#[async_trait]
impl SearchSource for WindowPresetsSource {
    fn name(&self) -> &'static str {
        "window_presets"
    }

    fn prefix(&self) -> Option<&'static str> {
        Some("w/")
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let query = query.trim().to_lowercase();

        if query.is_empty() {
            return PRESETS.iter().take(limit).map(make_item).collect();
        }

        let mut scored: Vec<(i32, &crate::window_mgmt::presets::Preset)> = PRESETS
            .iter()
            .filter_map(|p| {
                let mut score: i32 = 0;
                if p.display_name.to_lowercase().contains(&query) {
                    score += 50;
                }
                for kw in p.keywords {
                    if kw.starts_with(query.as_str()) {
                        score += 30;
                        break;
                    } else if kw.contains(query.as_str()) {
                        score += 10;
                        break;
                    }
                }
                if score > 0 {
                    Some((score, p))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);
        scored.into_iter().map(|(_, p)| make_item(p)).collect()
    }
}

fn make_item(p: &crate::window_mgmt::presets::Preset) -> LauncherItem {
    LauncherItem {
        id: format!("preset:{}", p.name),
        title: p.display_name.into(),
        subtitle: Some("Window".into()),
        icon: Some("window".into()),
        kind: LauncherItemKind::WindowAction {
            action: WindowAction::Preset(p.name.into()),
        },
        score: 1.0,
        no_view: true,
        arguments: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn search_left_returns_left_presets() {
        let src = WindowPresetsSource;
        let items = src.search("left", 20).await;
        assert!(items.iter().any(|i| i.title == "Left Half"));
        assert!(items.iter().any(|i| i.title == "Left Third"));
    }
}
