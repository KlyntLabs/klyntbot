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

/// Browser name suffixes typically appended to window titles (pre-lowercased
/// to avoid allocating on every call to `extract_site_name`).
const BROWSER_SUFFIXES: &[&str] = &[
    " - google chrome",
    " - mozilla firefox",
    " - safari",
    " - arc",
    " - brave",
    " - vivaldi",
    " - microsoft edge",
    " - opera",
    " - chromium",
    " — mozilla firefox",
    " — safari",
    " - zen browser",
];

/// Known site keywords → domain display name.
/// Checked against the lowercased title. Order matters: first match wins.
/// Display names use the full domain so users can distinguish web vs local apps.
const KNOWN_SITES: &[(&str, &str)] = &[
    // Social
    ("facebook", "facebook.com"),
    ("messenger.com", "messenger.com"),
    ("instagram", "instagram.com"),
    ("twitter.com", "twitter.com"),
    ("x.com", "x.com"),
    ("tiktok", "tiktok.com"),
    ("linkedin", "linkedin.com"),
    ("threads.net", "threads.net"),
    ("mastodon", "mastodon.social"),
    ("bluesky", "bsky.app"),
    // Video & streaming
    ("youtube", "youtube.com"),
    ("netflix", "netflix.com"),
    ("twitch", "twitch.tv"),
    ("disney+", "disneyplus.com"),
    ("hulu", "hulu.com"),
    ("spotify", "spotify.com"),
    // Dev & productivity
    ("github", "github.com"),
    ("gitlab", "gitlab.com"),
    ("bitbucket", "bitbucket.org"),
    ("stackoverflow", "stackoverflow.com"),
    ("stack overflow", "stackoverflow.com"),
    ("linear", "linear.app"),
    ("jira", "jira.com"),
    ("notion", "notion.so"),
    ("figma", "figma.com"),
    ("vercel", "vercel.com"),
    ("netlify", "netlify.com"),
    ("railway", "railway.app"),
    ("supabase", "supabase.com"),
    ("planetscale", "planetscale.com"),
    ("aws.amazon", "aws.amazon.com"),
    ("aws console", "aws.amazon.com"),
    ("azure", "azure.com"),
    // AI
    ("claude", "claude.ai"),
    ("anthropic", "anthropic.com"),
    ("chatgpt", "chatgpt.com"),
    ("openai", "openai.com"),
    ("gemini", "gemini.google.com"),
    ("perplexity", "perplexity.ai"),
    ("copilot", "copilot.microsoft.com"),
    // Communication
    ("slack", "slack.com"),
    ("discord", "discord.com"),
    ("telegram", "telegram.org"),
    ("whatsapp", "whatsapp.com"),
    ("zoom", "zoom.us"),
    ("meet.google", "meet.google.com"),
    ("teams.microsoft", "teams.microsoft.com"),
    // Google suite
    ("google docs", "docs.google.com"),
    ("google sheets", "sheets.google.com"),
    ("google slides", "slides.google.com"),
    ("google drive", "drive.google.com"),
    ("gmail", "gmail.com"),
    ("calendar.google", "calendar.google.com"),
    ("google maps", "maps.google.com"),
    ("google.com/search", "google.com"),
    ("google", "google.com"),
    // Other
    ("reddit", "reddit.com"),
    ("wikipedia", "wikipedia.org"),
    ("medium", "medium.com"),
    ("hackernews", "news.ycombinator.com"),
    ("hacker news", "news.ycombinator.com"),
    ("news.ycombinator", "news.ycombinator.com"),
    ("amazon", "amazon.com"),
    ("ebay", "ebay.com"),
    ("shopify", "shopify.com"),
    ("stripe", "stripe.com"),
    ("paypal", "paypal.com"),
    ("crates.io", "crates.io"),
    ("docs.rs", "docs.rs"),
    ("npmjs", "npmjs.com"),
    ("pypi", "pypi.org"),
    ("localhost", "localhost"),
    ("127.0.0.1", "localhost"),
];

/// Strip notification badge prefixes like "(1) ", "(99+) " from window titles.
fn strip_notification_badge(title: &str) -> String {
    let trimmed = title.trim_start();
    if !trimmed.starts_with('(') {
        return title.to_string();
    }
    if let Some(close) = trimmed.find(')') {
        let inside = &trimmed[1..close];
        // Must be digits optionally followed by '+'
        let is_badge = inside
            .trim_end_matches('+')
            .chars()
            .all(|c| c.is_ascii_digit())
            && !inside.is_empty();
        if is_badge {
            let rest = trimmed[close + 1..].trim_start();
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    title.to_string()
}

/// Lookup a known site from the title (case-insensitive).
fn lookup_known_site(title: &str) -> Option<&'static str> {
    let lower = title.to_lowercase();
    for &(keyword, display) in KNOWN_SITES {
        if lower.contains(keyword) {
            return Some(display);
        }
    }
    None
}

/// The ID of the generic "browsing" fallback category (must match migration seed).
const BROWSING_CATEGORY_ID: &str = "browsing";

pub struct Categorizer {
    /// Cached categories loaded from DB
    categories: Vec<ActivityCategory>,
    /// Pre-computed lowercased title match patterns (derived from url_patterns).
    /// Each entry corresponds to a category, containing lowercased patterns
    /// with TLD suffixes stripped (e.g. "youtube.com" → "youtube").
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
                            .map(|p| Self::strip_tld(p).to_lowercase())
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Strip common TLD suffixes for title matching (e.g. "youtube.com" → "youtube").
    fn strip_tld(pattern: &str) -> &str {
        const SUFFIXES: &[&str] = &[
            ".com", ".org", ".net", ".io", ".co", ".dev", ".app", ".ai", ".tv", ".social", ".new",
        ];
        for suffix in SUFFIXES {
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
    /// - `"(1) Facebook"` (notification badge)
    ///
    /// Strategy:
    /// 1. Strip the browser suffix (e.g. ` - Google Chrome`)
    /// 2. Strip notification badges like `(1)`, `(99+)`
    /// 3. Check against known-sites lookup for reliable identification
    /// 4. Take the last segment after ` - ` as the site name
    /// 5. If only one segment remains, use that as the site name
    pub fn extract_site_name(window_title: &str) -> String {
        let mut title = window_title;

        // Strip browser suffix (case-insensitive check using pre-lowercased suffixes)
        let title_lower = title.to_lowercase();
        for suffix in BROWSER_SUFFIXES {
            if let Some(pos) = title_lower.rfind(suffix) {
                title = &title[..pos];
                break;
            }
        }

        let title = title.trim();
        if title.is_empty() {
            return window_title.to_string();
        }

        // Strip notification badges: "(1) Title" → "Title", "(99+) Title" → "Title"
        let title = strip_notification_badge(title);

        // Try known-sites lookup first — matches keywords anywhere in the title
        if let Some(site) = lookup_known_site(&title) {
            return site.to_string();
        }

        // Split on common separators and take the last meaningful segment.
        // "Page Title - Site Name" → "Site Name"
        // "Site Name" → "Site Name"
        // "r/rust - Reddit" → "Reddit"
        for sep in &[" - ", " — ", " | "] {
            if let Some(pos) = title.rfind(sep) {
                let last_segment = title[pos + sep.len()..].trim();
                if !last_segment.is_empty() {
                    // Check if the extracted segment matches a known site
                    if let Some(site) = lookup_known_site(last_segment) {
                        return site.to_string();
                    }
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

        // Non-browser apps: standard matching by bundle_id → app_name → url → title
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

        // YouTube in Chrome → entertainment (not browsing!)
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

        // Claude in Chrome → ai_tools (not browsing!)
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

        // Unknown site in Chrome → falls back to browsing
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
        // Standard Chrome titles → domain names
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
            Categorizer::extract_site_name("Claude — Safari"),
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
