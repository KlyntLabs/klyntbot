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
        guard.whitelist_for_session(&key);
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
                last_used_at: r.last_used_at.to_rfc3339(),
                created_at: r.created_at.to_rfc3339(),
            })
            .collect())
    }

    pub async fn distraction_delete_rule(&self, id: i64) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.learned_rules.delete(id).await.map_err(map_prod_err)?;
        Ok(())
    }
}
