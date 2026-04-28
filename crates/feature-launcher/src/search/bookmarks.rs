use crate::types::*;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct BookmarkEntry {
    title: String,
    url: String,
}

#[derive(Clone)]
pub struct BookmarksSource {
    bookmarks: Arc<RwLock<Vec<BookmarkEntry>>>,
    browser: String,
}

impl BookmarksSource {
    pub fn new(browser: String) -> Self {
        Self {
            bookmarks: Arc::new(RwLock::new(Vec::new())),
            browser,
        }
    }

    pub fn browser_bookmarks_path(browser: &str) -> Option<PathBuf> {
        super::chromium_profile_dir(browser).map(|d| d.join("Bookmarks"))
    }

    fn parse_chromium_bookmarks(path: &std::path::Path) -> Vec<BookmarkEntry> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(j) => j,
            Err(_) => return vec![],
        };

        let mut entries = Vec::new();
        if let Some(roots) = json.get("roots") {
            for (_key, folder) in roots.as_object().into_iter().flatten() {
                Self::collect_bookmarks(folder, &mut entries);
            }
        }
        entries
    }

    fn collect_bookmarks(node: &serde_json::Value, entries: &mut Vec<BookmarkEntry>) {
        match node.get("type").and_then(|t| t.as_str()) {
            Some("url") => {
                if let (Some(title), Some(url)) = (
                    node.get("name").and_then(|n| n.as_str()),
                    node.get("url").and_then(|u| u.as_str()),
                ) {
                    entries.push(BookmarkEntry {
                        title: title.to_string(),
                        url: url.to_string(),
                    });
                }
            }
            Some("folder") => {
                if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
                    for child in children {
                        Self::collect_bookmarks(child, entries);
                    }
                }
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for BookmarksSource {
    fn name(&self) -> &'static str {
        "bookmarks"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let bookmarks = self.bookmarks.read();
        let scored = super::fuzzy_match(query, &bookmarks, |b| &b.title, limit);

        scored
            .into_iter()
            .map(|(score, b)| LauncherItem {
                id: format!("bookmark:{}", b.url),
                title: b.title.clone(),
                subtitle: Some(b.url.clone()),
                icon: Some("bookmark".to_string()),
                kind: LauncherItemKind::Bookmark {
                    url: b.url.clone(),
                    browser: self.browser.clone(),
                },
                score: (score as f64) / 1000.0 * 0.8,
                no_view: false,
                arguments: vec![],
                pinned: false,
            })
            .collect()
    }

    async fn refresh(&self) {
        let path = match Self::browser_bookmarks_path(&self.browser) {
            Some(p) if p.exists() => p,
            _ => {
                tracing::debug!("Bookmarks file not found for browser: {}", self.browser);
                return;
            }
        };
        let bookmarks = Self::parse_chromium_bookmarks(&path);
        tracing::info!(
            "Indexed {} bookmarks from {}",
            bookmarks.len(),
            self.browser
        );
        *self.bookmarks.write() = bookmarks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chromium_bookmarks() {
        let json = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "children": [
                        { "type": "url", "name": "Rust", "url": "https://rust-lang.org" },
                        { "type": "folder", "children": [
                            { "type": "url", "name": "Tokio", "url": "https://tokio.rs" }
                        ], "type": "folder" }
                    ]
                }
            }
        }"#;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("Bookmarks");
        std::fs::write(&path, json).unwrap();

        let entries = BookmarksSource::parse_chromium_bookmarks(&path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Rust");
        assert_eq!(entries[1].title, "Tokio");
    }
}
