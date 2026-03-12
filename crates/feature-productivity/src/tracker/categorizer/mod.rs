mod browser;

pub use browser::{extract_site_name, is_browser};

use browser::*;

use crate::repos::ActivityCategoryRepo;
use crate::types::ActivityCategory;

/// The ID of the generic "browsing" fallback category (must match migration seed).
const BROWSING_CATEGORY_ID: &str = "browsing";

pub struct Categorizer {
    /// Cached categories loaded from DB
    categories: Vec<ActivityCategory>,
    /// Pre-computed lowercased title match patterns (derived from url_patterns).
    /// Each entry corresponds to a category, containing lowercased patterns
    /// with TLD suffixes stripped (e.g. "youtube.com" -> "youtube").
    title_patterns: Vec<Vec<String>>,
    /// Cached index of the "browsing" fallback category (O(1) lookup).
    browsing_idx: Option<usize>,
}

impl Categorizer {
    pub fn new(categories: Vec<ActivityCategory>) -> Self {
        let title_patterns = Self::build_title_patterns(&categories);
        let browsing_idx = categories.iter().position(|c| c.id == BROWSING_CATEGORY_ID);
        Self {
            categories,
            title_patterns,
            browsing_idx,
        }
    }

    /// Reload categories from DB
    pub async fn refresh(&mut self, repo: &ActivityCategoryRepo) -> common::Result<()> {
        self.categories = repo.list_all().await?;
        self.title_patterns = Self::build_title_patterns(&self.categories);
        self.browsing_idx = self
            .categories
            .iter()
            .position(|c| c.id == BROWSING_CATEGORY_ID);
        Ok(())
    }

    /// Pre-compute lowercased patterns for window title matching.
    fn build_title_patterns(categories: &[ActivityCategory]) -> Vec<Vec<String>> {
        categories
            .iter()
            .map(|cat| {
                cat.rules
                    .as_ref()
                    .map(|rules| {
                        rules
                            .url_patterns
                            .iter()
                            .map(|p| strip_tld(p).to_lowercase())
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Check whether an app is a known browser by name or bundle ID.
    pub fn is_browser(app_name: &str, bundle_id: Option<&str>) -> bool {
        is_browser(app_name, bundle_id)
    }

    /// Extract a human-readable site name from a browser window title.
    pub fn extract_site_name(window_title: &str) -> String {
        extract_site_name(window_title)
    }

    /// Match an app to a category using rules.
    ///
    /// `window_title` is checked against `url_patterns` as a fallback — on macOS
    /// we can't read browser URLs, but the window title often contains the site
    /// name (e.g. "YouTube - Google Chrome").
    pub fn categorize(
        &self,
        app_name: &str,
        bundle_id: Option<&str>,
        url: Option<&str>,
    ) -> Option<&ActivityCategory> {
        self.categorize_full(app_name, bundle_id, url, None)
    }

    /// Extended categorization that also checks window title against url_patterns.
    ///
    /// For browsers, site-specific categories (matched by URL/title patterns) take
    /// priority over the generic "browsing" category. This ensures that YouTube in
    /// Chrome is categorized as "entertainment" (distracting), not "browsing" (neutral).
    pub fn categorize_full(
        &self,
        app_name: &str,
        bundle_id: Option<&str>,
        url: Option<&str>,
        window_title: Option<&str>,
    ) -> Option<&ActivityCategory> {
        let is_browser = Self::is_browser(app_name, bundle_id);

        // For browsers, try site-specific matching first (URL and title patterns).
        // This prevents the generic "browsing" category (matched by app_name)
        // from short-circuiting more specific categories like "entertainment".
        if is_browser {
            let title_lower = window_title.map(|t| t.to_lowercase());
            for (idx, cat) in self.categories.iter().enumerate() {
                // Skip the generic "browsing" fallback in this pass
                if Some(idx) == self.browsing_idx {
                    continue;
                }
                if let Some(ref rules) = cat.rules {
                    // Check URL patterns against actual URL
                    if let Some(u) = url {
                        if rules.url_patterns.iter().any(|p| u.contains(p)) {
                            return Some(cat);
                        }
                    }
                    // Check pre-computed patterns against window title
                    if let Some(ref tl) = title_lower {
                        if let Some(patterns) = self.title_patterns.get(idx) {
                            if patterns.iter().any(|p| tl.contains(p.as_str())) {
                                return Some(cat);
                            }
                        }
                    }
                }
            }
            // No site-specific match — fall back to "browsing" via cached index
            return self.browsing_idx.map(|i| &self.categories[i]);
        }

        // Non-browser apps: standard matching by bundle_id -> app_name -> url -> title
        for (idx, cat) in self.categories.iter().enumerate() {
            if let Some(ref rules) = cat.rules {
                if let Some(bid) = bundle_id {
                    if rules.bundle_ids.iter().any(|r| bid.eq_ignore_ascii_case(r)) {
                        return Some(cat);
                    }
                }
                if rules
                    .app_names
                    .iter()
                    .any(|r| app_name.eq_ignore_ascii_case(r))
                {
                    return Some(cat);
                }
                if let Some(u) = url {
                    if rules.url_patterns.iter().any(|p| u.contains(p)) {
                        return Some(cat);
                    }
                }
                if let Some(title) = window_title {
                    let title_lower = title.to_lowercase();
                    if let Some(patterns) = self.title_patterns.get(idx) {
                        if patterns.iter().any(|p| title_lower.contains(p.as_str())) {
                            return Some(cat);
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CategoryRules, CategoryType};

    #[test]
    fn test_categorize_by_app_name() {
        let categories = vec![ActivityCategory {
            id: "coding".into(),
            name: "Coding".into(),
            category_type: CategoryType::Productive,
            color: None,
            icon: None,
            rules: Some(CategoryRules {
                app_names: vec!["Visual Studio Code".into(), "Terminal".into()],
                bundle_ids: vec![],
                url_patterns: vec![],
            }),
            is_system: true,
        }];
        let cat = Categorizer::new(categories);
        assert_eq!(
            cat.categorize("Visual Studio Code", None, None).unwrap().id,
            "coding"
        );
        assert!(cat.categorize("Unknown App", None, None).is_none());
    }

    #[test]
    fn test_categorize_by_bundle_id() {
        let categories = vec![ActivityCategory {
            id: "coding".into(),
            name: "Coding".into(),
            category_type: CategoryType::Productive,
            color: None,
            icon: None,
            rules: Some(CategoryRules {
                app_names: vec![],
                bundle_ids: vec!["com.microsoft.VSCode".into()],
                url_patterns: vec![],
            }),
            is_system: true,
        }];
        let cat = Categorizer::new(categories);
        assert_eq!(
            cat.categorize("Code", Some("com.microsoft.VSCode"), None)
                .unwrap()
                .id,
            "coding"
        );
    }

    #[test]
    fn test_categorize_by_url() {
        let categories = vec![ActivityCategory {
            id: "entertainment".into(),
            name: "Entertainment".into(),
            category_type: CategoryType::Distracting,
            color: None,
            icon: None,
            rules: Some(CategoryRules {
                app_names: vec![],
                bundle_ids: vec![],
                url_patterns: vec!["youtube.com".into(), "reddit.com".into()],
            }),
            is_system: true,
        }];
        let cat = Categorizer::new(categories);
        assert_eq!(
            cat.categorize("Safari", None, Some("https://www.youtube.com/watch?v=abc"))
                .unwrap()
                .id,
            "entertainment"
        );
        assert!(cat
            .categorize("Safari", None, Some("https://github.com"))
            .is_none());
    }

    #[test]
    fn test_categorize_by_window_title() {
        let categories = vec![ActivityCategory {
            id: "entertainment".into(),
            name: "Entertainment".into(),
            category_type: CategoryType::Distracting,
            color: None,
            icon: None,
            rules: Some(CategoryRules {
                app_names: vec![],
                bundle_ids: vec![],
                url_patterns: vec!["youtube.com".into(), "reddit.com".into()],
            }),
            is_system: true,
        }];
        let cat = Categorizer::new(categories);

        // Browser window title contains the site name
        assert_eq!(
            cat.categorize_full(
                "Google Chrome",
                Some("com.google.Chrome"),
                None,
                Some("Some Video - YouTube - Google Chrome"),
            )
            .unwrap()
            .id,
            "entertainment"
        );

        // Reddit in window title
        assert_eq!(
            cat.categorize_full(
                "Google Chrome",
                None,
                None,
                Some("r/rust - Reddit - Google Chrome"),
            )
            .unwrap()
            .id,
            "entertainment"
        );

        // Non-distracting site in Chrome — no match (no "browsing" category in this test)
        assert!(cat
            .categorize_full(
                "Google Chrome",
                None,
                None,
                Some("GitHub - Where the world builds software"),
            )
            .is_none());
    }

    #[test]
    fn test_browser_site_specific_over_browsing() {
        // The key fix: site-specific categories must take priority over the
        // generic "browsing" category for browsers. Previously, Chrome always
        // matched "browsing" by app_name before title patterns were checked.
        let categories = vec![
            ActivityCategory {
                id: "browsing".into(),
                name: "Browsing".into(),
                category_type: CategoryType::Neutral,
                color: None,
                icon: None,
                rules: Some(CategoryRules {
                    app_names: vec!["Google Chrome".into(), "Safari".into()],
                    bundle_ids: vec!["com.google.Chrome".into()],
                    url_patterns: vec![],
                }),
                is_system: true,
            },
            ActivityCategory {
                id: "entertainment".into(),
                name: "Entertainment".into(),
                category_type: CategoryType::Distracting,
                color: None,
                icon: None,
                rules: Some(CategoryRules {
                    app_names: vec![],
                    bundle_ids: vec![],
                    url_patterns: vec!["youtube.com".into(), "netflix.com".into()],
                }),
                is_system: true,
            },
            ActivityCategory {
                id: "ai_tools".into(),
                name: "AI Tools".into(),
                category_type: CategoryType::Productive,
                color: None,
                icon: None,
                rules: Some(CategoryRules {
                    app_names: vec![],
                    bundle_ids: vec![],
                    url_patterns: vec!["claude.ai".into(), "chatgpt.com".into()],
                }),
                is_system: true,
            },
        ];
        let cat = Categorizer::new(categories);

        // YouTube in Chrome -> entertainment (not browsing!)
        assert_eq!(
            cat.categorize_full(
                "Google Chrome",
                Some("com.google.Chrome"),
                None,
                Some("Some Video - YouTube - Google Chrome"),
            )
            .unwrap()
            .id,
            "entertainment"
        );

        // Claude in Chrome -> ai_tools (not browsing!)
        assert_eq!(
            cat.categorize_full(
                "Google Chrome",
                Some("com.google.Chrome"),
                None,
                Some("Chat - Claude - Google Chrome"),
            )
            .unwrap()
            .id,
            "ai_tools"
        );

        // Unknown site in Chrome -> falls back to browsing
        assert_eq!(
            cat.categorize_full(
                "Google Chrome",
                Some("com.google.Chrome"),
                None,
                Some("Some Random Page - Google Chrome"),
            )
            .unwrap()
            .id,
            "browsing"
        );

        // Non-browser app still matches normally by app_name
        assert!(cat.categorize_full("Slack", None, None, None).is_none());
    }

    #[test]
    fn test_is_browser() {
        assert!(Categorizer::is_browser("Google Chrome", None));
        assert!(Categorizer::is_browser("Safari", None));
        assert!(Categorizer::is_browser("Arc", None));
        assert!(Categorizer::is_browser(
            "Firefox",
            Some("org.mozilla.firefox")
        ));
        assert!(!Categorizer::is_browser("Visual Studio Code", None));
        assert!(!Categorizer::is_browser("Slack", None));
        // Bundle ID detection
        assert!(Categorizer::is_browser(
            "Chrome Canary",
            Some("com.google.chrome.canary")
        ));
    }

    #[test]
    fn test_extract_site_name() {
        // Standard Chrome titles -> domain names
        assert_eq!(
            Categorizer::extract_site_name("Some Video - YouTube - Google Chrome"),
            "youtube.com"
        );
        assert_eq!(
            Categorizer::extract_site_name("r/rust - Reddit - Google Chrome"),
            "reddit.com"
        );
        assert_eq!(
            Categorizer::extract_site_name(
                "anthropics/claude-code: CLI for Claude - GitHub - Google Chrome"
            ),
            "github.com"
        );

        // Single segment (no separator after stripping browser)
        assert_eq!(
            Categorizer::extract_site_name("YouTube - Google Chrome"),
            "youtube.com"
        );

        // Safari with em-dash
        assert_eq!(
            Categorizer::extract_site_name("Claude \u{2014} Safari"),
            "claude.ai"
        );

        // Title with pipe separator
        assert_eq!(
            Categorizer::extract_site_name("Dashboard | Linear - Google Chrome"),
            "linear.app"
        );

        // No browser suffix (unknown browser or stripped title)
        assert_eq!(
            Categorizer::extract_site_name("Some Page - Some Site"),
            "Some Site"
        );

        // Notification badges stripped
        assert_eq!(
            Categorizer::extract_site_name("(1) Facebook - Google Chrome"),
            "facebook.com"
        );
        assert_eq!(
            Categorizer::extract_site_name("(99+) Slack - Google Chrome"),
            "slack.com"
        );

        // Unknown titles stay as-is
        assert_eq!(
            Categorizer::extract_site_name("1.4 GB - Google Chrome"),
            "1.4 GB"
        );

        // Known-site lookup from ambiguous titles
        assert_eq!(
            Categorizer::extract_site_name("Part of group - Claude (MCP) - Google Chrome"),
            "claude.ai"
        );

        // Known sites without separator
        assert_eq!(
            Categorizer::extract_site_name("Facebook - Google Chrome"),
            "facebook.com"
        );
        assert_eq!(
            Categorizer::extract_site_name("ChatGPT - Google Chrome"),
            "chatgpt.com"
        );
    }

    #[test]
    fn test_strip_notification_badge() {
        assert_eq!(strip_notification_badge("(1) Facebook"), "Facebook");
        assert_eq!(strip_notification_badge("(99+) Slack"), "Slack");
        assert_eq!(strip_notification_badge("(3) Messages"), "Messages");
        // Not a badge
        assert_eq!(strip_notification_badge("(abc) Title"), "(abc) Title");
        assert_eq!(strip_notification_badge("Normal Title"), "Normal Title");
        // Empty after stripping
        assert_eq!(strip_notification_badge("(5)"), "(5)");
    }

    #[test]
    fn test_lookup_known_site() {
        assert_eq!(lookup_known_site("YouTube"), Some("youtube.com"));
        assert_eq!(
            lookup_known_site("something youtube something"),
            Some("youtube.com")
        );
        assert_eq!(lookup_known_site("My GitHub Profile"), Some("github.com"));
        assert_eq!(lookup_known_site("random page title"), None);
    }
}
