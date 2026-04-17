//! Distraction overlay handlers — dismiss, allow, learned rules.

use desktop_shared::commands::LearnedRuleResponse;
use desktop_shared::errors::ApiError;
use feature_productivity::distraction::DistractionInterceptor;

use crate::errors::map_prod_err;
use crate::state::AppCore;

impl AppCore {
    pub async fn distraction_dismiss(&self, app_name: String) -> Result<(), ApiError> {
        let focus_mgr = self.focus_manager()?;
        focus_mgr
            .record_distraction(&app_name)
            .await
            .map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn distraction_allow_temp(&self, pattern: String) -> Result<(), ApiError> {
        let interceptor = self.distraction_interceptor()?;
        let mut guard = interceptor.lock().await;
        guard.grant_temp_pass(&pattern);
        Ok(())
    }

    pub async fn distraction_allow_session(
        &self,
        app_name: String,
        window_title: Option<String>,
        classification: String,
    ) -> Result<(), ApiError> {
        let (key, pattern_type) =
            DistractionInterceptor::make_key(&app_name, window_title.as_deref());

        let interceptor = self.distraction_interceptor()?;
        let mut guard = interceptor.lock().await;

        // Whitelist the specific title key (exact match)
        guard.whitelist_for_session(&key);

        // Also whitelist the page content portion (before the site suffix).
        // e.g. "Rust Tutorial - YouTube" → whitelist "rust tutorial"
        // This survives minor title changes while keeping it content-specific
        // (other YouTube videos won't be whitelisted).
        if let Some(ref title) = window_title {
            if let Some(page_title) = extract_page_title(title) {
                guard.whitelist_for_session(&page_title);
            }
        }
        drop(guard);

        let repos = self.productivity_repos()?;
        repos
            .learned_rules
            .upsert_or_hit(&key, pattern_type, &classification)
            .await
            .map_err(map_prod_err)?;

        Ok(())
    }

    pub async fn distraction_learned_rules(&self) -> Result<Vec<LearnedRuleResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let rules = repos.learned_rules.list_all().await.map_err(map_prod_err)?;
        Ok(rules
            .into_iter()
            .map(|r| LearnedRuleResponse {
                id: r.id.unwrap_or(0),
                pattern: r.pattern,
                pattern_type: r.pattern_type,
                classification: r.classification,
                confidence: r.confidence,
                hit_count: r.hit_count,
                last_used_at: r.last_used_at.to_string(),
                created_at: r.created_at.to_string(),
            })
            .collect())
    }

    pub async fn distraction_delete_rule(&self, id: i64) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.learned_rules.delete(id).await.map_err(map_prod_err)?;
        Ok(())
    }
}

/// Extract the page content title from a browser window title.
/// Browser titles are typically "Page Title - SiteName - BrowserName".
/// Strips the browser suffix first, then takes content before the site name.
/// e.g. "Rust Tutorial - YouTube - Google Chrome" → "rust tutorial"
fn extract_page_title(title: &str) -> Option<String> {
    const BROWSER_SUFFIXES: &[&str] = &[
        "Google Chrome",
        "Mozilla Firefox",
        "Safari",
        "Arc",
        "Microsoft Edge",
        "Brave Browser",
        "Opera",
        "Vivaldi",
    ];

    // Strip browser suffix if present
    let mut cleaned = title;
    for suffix in BROWSER_SUFFIXES {
        for sep in [" - ", " \u{2014} "] {
            let tail = format!("{sep}{suffix}");
            if let Some(stripped) = cleaned.strip_suffix(&tail) {
                cleaned = stripped.trim();
                break;
            }
        }
    }

    // Now split on the last separator to get page title (before site name)
    for sep in [" - ", " | ", " \u{2014} "] {
        if let Some(idx) = cleaned.rfind(sep) {
            let page = cleaned[..idx].trim().to_lowercase();
            if page.len() >= 3 {
                return Some(page);
            }
        }
    }
    None
}
