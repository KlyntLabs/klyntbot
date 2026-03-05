use crate::repos::ActivityCategoryRepo;
use crate::types::ActivityCategory;

/// Known browser app names (lowercased for comparison).
const BROWSER_APPS: &[&str] = &[
    "google chrome",
    "safari",
    "firefox",
    "arc",
    "brave browser",
    "orion",
    "vivaldi",
    "microsoft edge",
    "opera",
    "chromium",
    "zen browser",
];

/// Known browser bundle ID prefixes.
const BROWSER_BUNDLE_PREFIXES: &[&str] = &[
    "com.google.chrome",
    "com.apple.safari",
    "org.mozilla.firefox",
    "company.thebrowser.browser", // Arc
    "com.brave.browser",
    "com.microsoft.edgemac",
    "com.operasoftware.opera",
    "com.vivaldi.vivaldi",
];

/// Browser name suffixes typically appended to window titles.
const BROWSER_SUFFIXES: &[&str] = &[
    " - Google Chrome",
    " - Mozilla Firefox",
    " - Safari",
    " - Arc",
    " - Brave",
    " - Vivaldi",
    " - Microsoft Edge",
    " - Opera",
    " - Chromium",
    " — Mozilla Firefox",
    " — Safari",
    " - Zen Browser",
];

pub struct Categorizer {
    /// Cached categories loaded from DB
    categories: Vec<ActivityCategory>,
    /// Pre-computed lowercased title match patterns (derived from url_patterns).
    /// Each entry corresponds to a category, containing lowercased patterns
    /// with TLD suffixes stripped (e.g. "youtube.com" → "youtube").
    title_patterns: Vec<Vec<String>>,
}

impl Categorizer {
    pub fn new(categories: Vec<ActivityCategory>) -> Self {
        let title_patterns = Self::build_title_patterns(&categories);
        Self {
            categories,
            title_patterns,
        }
    }

    /// Reload categories from DB
    pub async fn refresh(&mut self, repo: &ActivityCategoryRepo) -> common::Result<()> {
        self.categories = repo.list_all().await?;
        self.title_patterns = Self::build_title_patterns(&self.categories);
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
                            .map(|p| Self::strip_tld(p).to_lowercase())
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Strip common TLD suffixes for title matching (e.g. "youtube.com" → "youtube").
    fn strip_tld(pattern: &str) -> &str {
        let suffixes = [".com", ".org", ".net", ".io", ".co", ".dev", ".app"];
        for suffix in &suffixes {
            if let Some(stripped) = pattern.strip_suffix(suffix) {
                return stripped;
            }
        }
        pattern
    }

    /// Check whether an app is a known browser by name or bundle ID.
    pub fn is_browser(app_name: &str, bundle_id: Option<&str>) -> bool {
        let name_lower = app_name.to_lowercase();
        if BROWSER_APPS.iter().any(|b| name_lower == *b) {
            return true;
        }
        if let Some(bid) = bundle_id {
            let bid_lower = bid.to_lowercase();
            if BROWSER_BUNDLE_PREFIXES
                .iter()
                .any(|p| bid_lower.starts_with(p))
            {
                return true;
            }
        }
        false
    }

    /// Extract a human-readable site name from a browser window title.
    ///
    /// Browser titles typically look like:
    /// - `"Page Title - Site Name - Google Chrome"`
    /// - `"Claude - Anthropic - Google Chrome"`
    /// - `"r/rust - Reddit - Google Chrome"`
    ///
    /// Strategy:
    /// 1. Strip the browser suffix (e.g. ` - Google Chrome`)
    /// 2. Take the last segment after ` - ` as the site name (most browsers put
    ///    site name last, page title first)
    /// 3. If only one segment remains, use that as the site name
    pub fn extract_site_name(window_title: &str) -> String {
        let mut title = window_title;

        // Strip browser suffix (case-insensitive check)
        let title_lower = title.to_lowercase();
        for suffix in BROWSER_SUFFIXES {
            if let Some(pos) = title_lower.rfind(&suffix.to_lowercase()) {
                title = &title[..pos];
                break;
            }
        }

        let title = title.trim();
        if title.is_empty() {
            return window_title.to_string();
        }

        // Split on common separators and take the last meaningful segment.
        // "Page Title - Site Name" → "Site Name"
        // "Site Name" → "Site Name"
        // "r/rust - Reddit" → "Reddit"
        for sep in &[" - ", " — ", " | "] {
            if let Some(pos) = title.rfind(sep) {
                let last_segment = title[pos + sep.len()..].trim();
                if !last_segment.is_empty() {
                    return last_segment.to_string();
                }
            }
        }

        title.to_string()
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
    pub fn categorize_full(
        &self,
        app_name: &str,
        bundle_id: Option<&str>,
        url: Option<&str>,
        window_title: Option<&str>,
    ) -> Option<&ActivityCategory> {
        for (idx, cat) in self.categories.iter().enumerate() {
            if let Some(ref rules) = cat.rules {
                // Check bundle_id first (most specific)
                if let Some(bid) = bundle_id {
                    if rules.bundle_ids.iter().any(|r| bid.eq_ignore_ascii_case(r)) {
                        return Some(cat);
                    }
                }
                // Check app_name
                if rules
                    .app_names
                    .iter()
                    .any(|r| app_name.eq_ignore_ascii_case(r))
                {
                    return Some(cat);
                }
                // Check URL patterns against actual URL
                if let Some(u) = url {
                    if rules.url_patterns.iter().any(|p| u.contains(p)) {
                        return Some(cat);
                    }
                }
                // Check pre-computed patterns against window title (browsers include site name)
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

        // Non-distracting site in Chrome — no match
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
    fn test_is_browser() {
        assert!(Categorizer::is_browser("Google Chrome", None));
        assert!(Categorizer::is_browser("Safari", None));
        assert!(Categorizer::is_browser("Arc", None));
        assert!(Categorizer::is_browser("Firefox", Some("org.mozilla.firefox")));
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
        // Standard Chrome titles
        assert_eq!(
            Categorizer::extract_site_name("Some Video - YouTube - Google Chrome"),
            "YouTube"
        );
        assert_eq!(
            Categorizer::extract_site_name("r/rust - Reddit - Google Chrome"),
            "Reddit"
        );
        assert_eq!(
            Categorizer::extract_site_name(
                "anthropics/claude-code: CLI for Claude - GitHub - Google Chrome"
            ),
            "GitHub"
        );

        // Single segment (no separator after stripping browser)
        assert_eq!(
            Categorizer::extract_site_name("YouTube - Google Chrome"),
            "YouTube"
        );

        // Safari with em-dash
        assert_eq!(
            Categorizer::extract_site_name("Claude — Safari"),
            "Claude"
        );

        // Title with pipe separator
        assert_eq!(
            Categorizer::extract_site_name("Dashboard | Linear - Google Chrome"),
            "Linear"
        );

        // No browser suffix (unknown browser or stripped title)
        assert_eq!(
            Categorizer::extract_site_name("Some Page - Some Site"),
            "Some Site"
        );
    }
}
