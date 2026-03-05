//! Fast heuristic classifier for distraction content.
//! Returns a confident verdict for obvious cases, or `Ambiguous` for LLM fallback.

/// Classification result from the heuristic engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeuristicVerdict {
    /// Definitely distracting — show overlay immediately, no LLM needed.
    ConfidentDistracting,
    /// Definitely productive — skip overlay entirely.
    ConfidentProductive,
    /// Uncertain — show overlay and fire async LLM classification.
    Ambiguous,
}

/// Apps that are always distracting regardless of content (all lowercase for comparison).
const ALWAYS_DISTRACTING_APPS: &[&str] = &["netflix", "tiktok", "instagram", "twitch", "discord"];

/// Window title keywords that signal definite distraction.
/// Note: apps already in ALWAYS_DISTRACTING_APPS are omitted here since step 1 returns early.
const DISTRACTING_TITLE_KEYWORDS: &[&str] = &["facebook", "hacker news", "twitch.tv"];

/// Window title keywords that signal productive content.
const PRODUCTIVE_TITLE_KEYWORDS: &[&str] = &[
    "stack overflow",
    "stackoverflow",
    "mdn web docs",
    "docs.rs",
    "github issues",
    "github pull",
    "crates.io",
    "npm",
    "pypi",
    "developer.apple.com",
    "developer.mozilla",
    "cppreference",
    "arxiv",
    "google scholar",
    "coursera",
    "udemy",
    "edx",
];

/// Apps whose content is inherently ambiguous (all lowercase for comparison).
const AMBIGUOUS_APPS: &[&str] = &["youtube", "reddit"];

/// Classify a distraction alert using fast heuristics.
///
/// `app_name`: The frontmost application name (e.g. "Google Chrome", "YouTube").
/// `window_title`: The window title, often contains site name for browsers.
pub fn classify(app_name: &str, window_title: Option<&str>) -> HeuristicVerdict {
    // 1. Check always-distracting apps (by app name) — no allocation needed.
    if ALWAYS_DISTRACTING_APPS
        .iter()
        .any(|a| a.eq_ignore_ascii_case(app_name))
    {
        return HeuristicVerdict::ConfidentDistracting;
    }

    // Allocate lowercased title only when needed for substring matching.
    let title_lower = window_title.map(|t| t.to_lowercase()).unwrap_or_default();

    // 2. Check productive title keywords — takes priority over distracting.
    if PRODUCTIVE_TITLE_KEYWORDS
        .iter()
        .any(|k| title_lower.contains(k))
    {
        return HeuristicVerdict::ConfidentProductive;
    }

    // 3. Check distracting title keywords.
    if DISTRACTING_TITLE_KEYWORDS
        .iter()
        .any(|k| title_lower.contains(k))
    {
        return HeuristicVerdict::ConfidentDistracting;
    }

    // 4. Check ambiguous apps — these need LLM classification.
    let app_lower = app_name.to_ascii_lowercase();
    if AMBIGUOUS_APPS
        .iter()
        .any(|a| app_lower.contains(a) || title_lower.contains(a))
    {
        return HeuristicVerdict::Ambiguous;
    }

    // 5. Default: if the categorizer already flagged it as distracting
    //    but we don't recognize the pattern, treat as ambiguous.
    HeuristicVerdict::Ambiguous
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_distracting_apps() {
        assert_eq!(
            classify("Netflix", None),
            HeuristicVerdict::ConfidentDistracting
        );
        assert_eq!(
            classify("TikTok", None),
            HeuristicVerdict::ConfidentDistracting
        );
        assert_eq!(
            classify("Instagram", Some("Feed")),
            HeuristicVerdict::ConfidentDistracting
        );
    }

    #[test]
    fn productive_titles_override() {
        assert_eq!(
            classify("Google Chrome", Some("How to use Rust - Stack Overflow")),
            HeuristicVerdict::ConfidentProductive,
        );
        assert_eq!(
            classify("Safari", Some("std::vec - docs.rs")),
            HeuristicVerdict::ConfidentProductive,
        );
    }

    #[test]
    fn distracting_title_keywords() {
        assert_eq!(
            classify("Google Chrome", Some("r/memes - Facebook")),
            HeuristicVerdict::ConfidentDistracting,
        );
    }

    #[test]
    fn youtube_is_ambiguous() {
        assert_eq!(
            classify("Google Chrome", Some("Funny cats compilation - YouTube")),
            HeuristicVerdict::Ambiguous,
        );
        assert_eq!(
            classify("YouTube", Some("Rust async deep dive")),
            HeuristicVerdict::Ambiguous,
        );
    }

    #[test]
    fn reddit_is_ambiguous() {
        assert_eq!(
            classify("Arc", Some("r/rust - Best practices - Reddit")),
            HeuristicVerdict::Ambiguous,
        );
    }

    #[test]
    fn unknown_distracting_app_is_ambiguous() {
        assert_eq!(
            classify("SomeRandomApp", Some("random content")),
            HeuristicVerdict::Ambiguous,
        );
    }
}
