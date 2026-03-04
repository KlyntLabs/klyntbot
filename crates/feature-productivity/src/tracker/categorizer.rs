use crate::repos::ActivityCategoryRepo;
use crate::types::ActivityCategory;

pub struct Categorizer {
    /// Cached categories loaded from DB
    categories: Vec<ActivityCategory>,
}

impl Categorizer {
    pub fn new(categories: Vec<ActivityCategory>) -> Self {
        Self { categories }
    }

    /// Reload categories from DB
    pub async fn refresh(&mut self, repo: &ActivityCategoryRepo) -> common::Result<()> {
        self.categories = repo.list_all().await?;
        Ok(())
    }

    /// Match an app to a category using rules
    pub fn categorize(
        &self,
        app_name: &str,
        bundle_id: Option<&str>,
        url: Option<&str>,
    ) -> Option<&ActivityCategory> {
        for cat in &self.categories {
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
                // Check URL patterns
                if let Some(u) = url {
                    if rules.url_patterns.iter().any(|p| u.contains(p)) {
                        return Some(cat);
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
        assert!(cat.categorize("Safari", None, Some("https://github.com")).is_none());
    }
}
