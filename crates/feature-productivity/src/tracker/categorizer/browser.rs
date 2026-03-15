/// Browser name suffixes typically appended to window titles (pre-lowercased
/// to avoid allocating on every call to `extract_site_name`).
pub(super) const BROWSER_SUFFIXES: &[&str] = &[
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

/// Known site keywords -> domain display name.
/// Checked against the lowercased title. Order matters: first match wins.
/// Display names use the full domain so users can distinguish web vs local apps.
pub(super) const KNOWN_SITES: &[(&str, &str)] = &[
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
pub(super) fn strip_notification_badge(title: &str) -> String {
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
pub(super) fn lookup_known_site(title: &str) -> Option<&'static str> {
    let lower = title.to_lowercase();
    for &(keyword, display) in KNOWN_SITES {
        if lower.contains(keyword) {
            return Some(display);
        }
    }
    None
}

/// Check whether an app is a known browser by name or bundle ID.
///
/// Delegates to the shared `platform-macos` browser registry.
pub fn is_browser(app_name: &str, bundle_id: Option<&str>) -> bool {
    platform_macos::browser::is_browser(app_name, bundle_id)
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

    // Strip notification badges: "(1) Title" -> "Title", "(99+) Title" -> "Title"
    let title = strip_notification_badge(title);

    // Try known-sites lookup first — matches keywords anywhere in the title
    if let Some(site) = lookup_known_site(&title) {
        return site.to_string();
    }

    // Split on common separators and take the last meaningful segment.
    // "Page Title - Site Name" -> "Site Name"
    // "Site Name" -> "Site Name"
    // "r/rust - Reddit" -> "Reddit"
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

/// Strip common TLD suffixes for title matching (e.g. "youtube.com" -> "youtube").
pub(super) fn strip_tld(pattern: &str) -> &str {
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
