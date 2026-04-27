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
        let query = query.trim();
        if query.is_empty() {
            return PRESETS.iter().take(limit).map(make_item).collect();
        }

        let mut scored: Vec<(i32, &crate::window_mgmt::presets::Preset)> = PRESETS
            .iter()
            .filter_map(|p| {
                let mut score: i32 = 0;
                if contains_ascii_ci(p.display_name, query) {
                    score += 50;
                }
                for kw in p.keywords {
                    if starts_with_ascii_ci(kw, query) {
                        score += 30;
                        break;
                    } else if contains_ascii_ci(kw, query) {
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

fn starts_with_ascii_ci(haystack: &str, needle: &str) -> bool {
    haystack.len() >= needle.len()
        && haystack.as_bytes()[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
}

fn contains_ascii_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
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
    pinned: false,
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
