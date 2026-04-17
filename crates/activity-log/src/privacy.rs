use crate::types::ActivityLogEntry;

/// Privacy filter that excludes or flags sensitive activity events.
pub struct PrivacyFilter {
    excluded_apps: Vec<String>,
    excluded_url_patterns: Vec<String>,
    sensitive_apps: Vec<String>,
    sensitive_url_patterns: Vec<String>,
}

impl Default for PrivacyFilter {
    fn default() -> Self {
        Self {
            excluded_apps: vec![],
            excluded_url_patterns: vec![],
            sensitive_apps: vec![
                "1Password".to_string(),
                "Keychain Access".to_string(),
                "LastPass".to_string(),
                "Bitwarden".to_string(),
            ],
            sensitive_url_patterns: vec![
                "bank".to_string(),
                "banking".to_string(),
                "chase.com".to_string(),
                "wellsfargo.com".to_string(),
                "paypal.com".to_string(),
            ],
        }
    }
}

/// Check if `haystack` case-insensitively contains any pattern.
fn matches_any(haystack: &str, patterns: &[String]) -> bool {
    let lower = haystack.to_lowercase();
    patterns
        .iter()
        .any(|pat| lower.contains(&pat.to_lowercase()))
}

impl PrivacyFilter {
    pub fn new(
        excluded_apps: Vec<String>,
        excluded_url_patterns: Vec<String>,
        sensitive_apps: Vec<String>,
        sensitive_url_patterns: Vec<String>,
    ) -> Self {
        Self {
            excluded_apps,
            excluded_url_patterns,
            sensitive_apps,
            sensitive_url_patterns,
        }
    }

    /// Returns true if this event should be completely excluded from the log.
    pub fn should_exclude(&self, entry: &ActivityLogEntry) -> bool {
        if entry
            .app_name
            .as_deref()
            .is_some_and(|app| matches_any(app, &self.excluded_apps))
        {
            return true;
        }
        entry
            .resource_id
            .as_deref()
            .is_some_and(|url| matches_any(url, &self.excluded_url_patterns))
    }

    /// Flag entry as sensitive if it matches known sensitive patterns.
    pub fn flag_sensitive(&self, mut entry: ActivityLogEntry) -> ActivityLogEntry {
        if entry
            .app_name
            .as_deref()
            .is_some_and(|app| matches_any(app, &self.sensitive_apps))
            || entry
                .resource_id
                .as_deref()
                .is_some_and(|url| matches_any(url, &self.sensitive_url_patterns))
        {
            entry.is_sensitive = true;
        }
        entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalizers::new_ulid;
    use crate::types::{ActivityActor, ActivitySource};
    use jiff::Timestamp;

    fn make_entry(app: Option<&str>, resource_id: Option<&str>) -> ActivityLogEntry {
        ActivityLogEntry {
            id: new_ulid(),
            timestamp: Timestamp::now(),
            source: ActivitySource::OsWindow,
            actor: ActivityActor::User,
            resource_type: None,
            resource_id: resource_id.map(String::from),
            resource_name: None,
            action: "view".into(),
            content_preview: None,
            content_hash: None,
            metadata: None,
            app_name: app.map(String::from),
            project_id: None,
            work_context_id: None,
            embedding_id: None,
            duration_secs: None,
            session_key: None,
            is_sensitive: false,
        }
    }

    #[test]
    fn test_exclude_by_app() {
        let filter = PrivacyFilter::new(vec!["1Password".into()], vec![], vec![], vec![]);
        let entry = make_entry(Some("1Password 7"), None);
        assert!(filter.should_exclude(&entry));
    }

    #[test]
    fn test_exclude_by_url() {
        let filter = PrivacyFilter::new(vec![], vec!["secret.internal".into()], vec![], vec![]);
        let entry = make_entry(None, Some("https://secret.internal/admin"));
        assert!(filter.should_exclude(&entry));
    }

    #[test]
    fn test_no_exclude_normal_app() {
        let filter = PrivacyFilter::default();
        let entry = make_entry(Some("Visual Studio Code"), None);
        assert!(!filter.should_exclude(&entry));
    }

    #[test]
    fn test_flag_sensitive_banking_url() {
        let filter = PrivacyFilter::default();
        let entry = make_entry(None, Some("https://www.chase.com/account"));
        let flagged = filter.flag_sensitive(entry);
        assert!(flagged.is_sensitive);
    }

    #[test]
    fn test_flag_sensitive_password_manager() {
        let filter = PrivacyFilter::default();
        let entry = make_entry(Some("1Password"), None);
        let flagged = filter.flag_sensitive(entry);
        assert!(flagged.is_sensitive);
    }

    #[test]
    fn test_no_sensitive_normal() {
        let filter = PrivacyFilter::default();
        let entry = make_entry(Some("Terminal"), None);
        let flagged = filter.flag_sensitive(entry);
        assert!(!flagged.is_sensitive);
    }
}
