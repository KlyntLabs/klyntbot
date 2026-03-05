use crate::repos::ActivityCategoryRepo;
use crate::types::ActivityCategory;

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
}
