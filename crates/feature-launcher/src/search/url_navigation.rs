use crate::types::{LauncherItem, LauncherItemKind};

/// Detects URL-like queries and returns a navigation item.
pub struct UrlNavigation;

impl UrlNavigation {
    /// If `query` looks like a URL, return a LauncherItem to open it in the browser.
    pub fn try_match(query: &str) -> Option<LauncherItem> {
        let url = normalize_url(query)?;
        Some(LauncherItem {
            id: format!("url:{url}"),
            title: format!("Open {query}"),
            subtitle: Some(url.clone()),
            icon: Some("globe".to_string()),
            kind: LauncherItemKind::UrlNavigation { url },
            score: 2.5,
            no_view: false,
        })
    }
}

/// Check if the query looks like a URL and normalize it with a scheme.
fn normalize_url(query: &str) -> Option<String> {
    let q = query.trim();
    if q.is_empty() || q.contains(' ') {
        return None;
    }

    // Already has a scheme (http://, https://, ftp://)
    if q.starts_with("http://") || q.starts_with("https://") || q.starts_with("ftp://") {
        return Some(q.to_string());
    }

    // Looks like a domain: contains a dot, first segment is not empty,
    // and the TLD part has 2+ alpha chars (e.g. x.com, docs.google.com, 192.168.1.1)
    if looks_like_domain(q) {
        return Some(format!("https://{q}"));
    }

    // localhost with optional port
    if q.starts_with("localhost") {
        return Some(format!("http://{q}"));
    }

    None
}

fn looks_like_domain(s: &str) -> bool {
    // Split off any path/query (take only the host part)
    let host = s.split('/').next().unwrap_or(s);
    let host = host.split('?').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host); // strip port

    // Must contain at least one dot
    if !host.contains('.') {
        return false;
    }

    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // All parts must be non-empty
    if parts.iter().any(|p| p.is_empty()) {
        return false;
    }

    // Check if it's an IP address (all parts are numeric)
    let all_numeric = parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()));
    if all_numeric {
        return parts.len() == 4; // IPv4
    }

    // TLD must be 2+ alpha characters
    let tld = parts.last().unwrap();
    tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_domains() {
        assert_eq!(normalize_url("x.com"), Some("https://x.com".into()));
        assert_eq!(
            normalize_url("github.com"),
            Some("https://github.com".into())
        );
        assert_eq!(
            normalize_url("docs.google.com"),
            Some("https://docs.google.com".into())
        );
    }

    #[test]
    fn test_domains_with_paths() {
        assert_eq!(
            normalize_url("github.com/user/repo"),
            Some("https://github.com/user/repo".into())
        );
    }

    #[test]
    fn test_full_urls() {
        assert_eq!(
            normalize_url("https://example.com"),
            Some("https://example.com".into())
        );
        assert_eq!(
            normalize_url("http://localhost:3000"),
            Some("http://localhost:3000".into())
        );
    }

    #[test]
    fn test_ip_addresses() {
        assert_eq!(
            normalize_url("192.168.1.1"),
            Some("https://192.168.1.1".into())
        );
    }

    #[test]
    fn test_localhost() {
        assert_eq!(
            normalize_url("localhost:3000"),
            Some("http://localhost:3000".into())
        );
        assert_eq!(normalize_url("localhost"), Some("http://localhost".into()));
    }

    #[test]
    fn test_not_urls() {
        assert_eq!(normalize_url("hello world"), None);
        assert_eq!(normalize_url("safari"), None);
        assert_eq!(normalize_url(""), None);
        assert_eq!(normalize_url("just-a-word"), None);
    }

    #[test]
    fn test_try_match() {
        let item = UrlNavigation::try_match("x.com").unwrap();
        assert_eq!(item.title, "Open x.com");
        assert!(
            matches!(item.kind, LauncherItemKind::UrlNavigation { url } if url == "https://x.com")
        );
    }
}
