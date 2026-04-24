use std::collections::{HashMap, HashSet};

use regex::Regex;
use tracing::warn;

use crate::repos::{PrivacyRuleRepo, RuleEvolutionLogRepo, TrackingRuleRepo};
use crate::types::*;

/// In-memory rule engine for O(1) app classification with fallback to
/// prefix/contains/regex scans. Loaded from the tracking_rules DB table.
pub struct TrackingRulesEngine {
    /// Exact app_name lookup (lowercased key -> best-priority rule).
    app_exact: HashMap<String, TrackingRule>,
    /// Non-regex, non-exact-app rules sorted by priority.
    other_rules: Vec<TrackingRule>,
    /// Compiled regex rules.
    regex_rules: Vec<(Regex, TrackingRule)>,

    // Privacy
    excluded_apps: HashSet<String>,
    redact_title_patterns: Vec<PrivacyRule>,
    exclude_url_patterns: Vec<PrivacyRule>,

    // Repos
    rule_repo: TrackingRuleRepo,
    privacy_repo: PrivacyRuleRepo,
    evolution_repo: RuleEvolutionLogRepo,
}

impl TrackingRulesEngine {
    /// Load all active rules and privacy rules from the database.
    pub async fn load(
        rule_repo: TrackingRuleRepo,
        privacy_repo: PrivacyRuleRepo,
        evolution_repo: RuleEvolutionLogRepo,
    ) -> common::Result<Self> {
        let mut engine = Self {
            app_exact: HashMap::new(),
            other_rules: Vec::new(),
            regex_rules: Vec::new(),
            excluded_apps: HashSet::new(),
            redact_title_patterns: Vec::new(),
            exclude_url_patterns: Vec::new(),
            rule_repo,
            privacy_repo,
            evolution_repo,
        };
        engine.reload_inner().await?;
        Ok(engine)
    }

    /// Reload rules from the database (after evolve_rule or external changes).
    pub async fn reload(&mut self) -> common::Result<()> {
        self.reload_inner().await
    }

    async fn reload_inner(&mut self) -> common::Result<()> {
        self.app_exact.clear();
        self.other_rules.clear();
        self.regex_rules.clear();
        self.excluded_apps.clear();
        self.redact_title_patterns.clear();
        self.exclude_url_patterns.clear();

        let rules = self.rule_repo.list_active().await?;
        for rule in rules {
            if rule.match_mode == MatchMode::Regex {
                match Regex::new(&rule.match_pattern) {
                    Ok(re) => self.regex_rules.push((re, rule)),
                    Err(e) => {
                        warn!(
                            rule_id = %rule.id,
                            pattern = %rule.match_pattern,
                            "skipping rule with invalid regex: {e}"
                        );
                    }
                }
            } else if rule.rule_type == RuleType::App && rule.match_mode == MatchMode::Exact {
                let key = rule.match_pattern.to_lowercase();
                self.app_exact
                    .entry(key)
                    .and_modify(|existing| {
                        if rule.priority < existing.priority {
                            *existing = rule.clone();
                        }
                    })
                    .or_insert(rule);
            } else {
                // Pre-lowercase match_pattern at load time to avoid per-tick allocations
                let mut rule = rule;
                rule.match_pattern = rule.match_pattern.to_lowercase();
                self.other_rules.push(rule);
            }
        }
        self.other_rules.sort_by_key(|r| r.priority);

        // Load privacy rules
        let privacy_rules = self.privacy_repo.list_all().await?;
        for rule in privacy_rules {
            match rule.rule_type.as_str() {
                "exclude_app" => {
                    self.excluded_apps.insert(rule.pattern.to_lowercase());
                }
                "redact_title" => self.redact_title_patterns.push(rule),
                "exclude_url" => self.exclude_url_patterns.push(rule),
                _ => warn!(rule_type = %rule.rule_type, "unknown privacy rule type"),
            }
        }

        Ok(())
    }

    /// Classify an activity tick into a category and session type.
    /// Returns Default source if no rule matches.
    pub fn classify(
        &self,
        app: &str,
        url: Option<&str>,
        title: Option<&str>,
    ) -> ClassificationResult {
        // 1. Exact app lookup O(1)
        if let Some(rule) = self.app_exact.get(&app.to_lowercase()) {
            return rule_to_result(rule);
        }

        // 2. Scan other rules (URL, Title, non-exact App) by priority
        // Lowercase the field value once before the loop (patterns are pre-lowercased at load time)
        let app_lower = app.to_lowercase();
        let url_lower = url.map(|u| u.to_lowercase());
        let title_lower = title.map(|t| t.to_lowercase());
        for rule in &self.other_rules {
            let field: Option<&str> = match rule.rule_type {
                RuleType::App => Some(&app_lower),
                RuleType::Url => url_lower.as_deref(),
                RuleType::Title => title_lower.as_deref(),
                RuleType::Compound => None,
            };
            if let Some(value) = field {
                if matches_pattern(value, &rule.match_pattern, rule.match_mode) {
                    return rule_to_result(rule);
                }
            }
        }

        // 3. Regex scan
        for (re, rule) in &self.regex_rules {
            let field = match rule.rule_type {
                RuleType::App => Some(app),
                RuleType::Url => url,
                RuleType::Title => title,
                RuleType::Compound => None,
            };
            if let Some(value) = field {
                if re.is_match(value) {
                    return rule_to_result(rule);
                }
            }
        }

        // 4. Default
        ClassificationResult {
            category: "uncategorized".to_string(),
            session_type: IntelligenceSessionType::Focus,
            confidence: 0.0,
            rule_id: None,
            source: ClassificationSource::Default,
        }
    }

    /// Apply privacy filters to determine if an app/title/url should be excluded or redacted.
    pub fn apply_privacy(
        &self,
        app: &str,
        title: Option<&str>,
        url: Option<&str>,
    ) -> PrivacyFilterResult {
        if self.excluded_apps.contains(&app.to_lowercase()) {
            return PrivacyFilterResult {
                excluded: true,
                title: None,
                url: None,
            };
        }

        if let Some(url_str) = url {
            for rule in &self.exclude_url_patterns {
                if matches_pattern(
                    &url_str.to_lowercase(),
                    &rule.pattern.to_lowercase(),
                    rule.match_mode,
                ) {
                    return PrivacyFilterResult {
                        excluded: true,
                        title: None,
                        url: None,
                    };
                }
            }
        }

        let redacted_title = title.and_then(|t| {
            let t_lower = t.to_lowercase();
            for rule in &self.redact_title_patterns {
                if matches_pattern(&t_lower, &rule.pattern.to_lowercase(), rule.match_mode) {
                    return None;
                }
            }
            Some(t.to_string())
        });

        PrivacyFilterResult {
            excluded: false,
            title: redacted_title,
            url: url.map(|u| u.to_string()),
        }
    }

    /// Insert or update a tracking rule and log the evolution, then reload.
    pub async fn evolve_rule(
        &mut self,
        rule: &TrackingRule,
        entry: &RuleEvolutionEntry,
    ) -> common::Result<()> {
        self.rule_repo.upsert(rule).await?;
        self.evolution_repo.create(entry).await?;
        self.reload().await
    }

    /// Record a hit for a matching rule.
    pub async fn record_hit(&self, rule_id: &str) -> common::Result<()> {
        self.rule_repo.increment_hit(rule_id).await
    }
}

fn rule_to_result(rule: &TrackingRule) -> ClassificationResult {
    ClassificationResult {
        category: rule.category.clone(),
        session_type: rule.session_type,
        confidence: rule.confidence,
        rule_id: Some(rule.id.clone()),
        source: ClassificationSource::Rule,
    }
}

fn matches_pattern(value: &str, pattern: &str, mode: MatchMode) -> bool {
    match mode {
        MatchMode::Exact => value == pattern,
        MatchMode::Prefix => value.starts_with(pattern),
        MatchMode::Contains => value.contains(pattern),
        MatchMode::Regex => false, // Handled separately via compiled regex
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{PrivacyRuleRepo, RuleEvolutionLogRepo, TrackingRuleRepo};
    use crate::ProductivityFeature;
    use jiff::Timestamp;
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    async fn setup_engine(pool: &SqlitePool) -> TrackingRulesEngine {
        TrackingRulesEngine::load(
            TrackingRuleRepo::new(pool.clone()),
            PrivacyRuleRepo::new(pool.clone()),
            RuleEvolutionLogRepo::new(pool.clone()),
        )
        .await
        .unwrap()
    }

    fn sample_rule(id: &str, pattern: &str, category: &str) -> TrackingRule {
        let now = Timestamp::now();
        TrackingRule {
            id: id.to_string(),
            rule_type: RuleType::App,
            match_field: "app_name".to_string(),
            match_pattern: pattern.to_string(),
            match_mode: MatchMode::Exact,
            category: category.to_string(),
            session_type: IntelligenceSessionType::Focus,
            priority: 10,
            source: RuleSource::User,
            confidence: 0.95,
            hit_count: 0,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_classify_exact_app_match() {
        let pool = setup_pool().await;
        sqlx::query("DELETE FROM productivity_tracking_rules")
            .execute(&pool)
            .await
            .unwrap();

        let rule_repo = TrackingRuleRepo::new(pool.clone());
        rule_repo
            .create(&sample_rule("test-app-1", "Code", "coding"))
            .await
            .unwrap();

        let engine = setup_engine(&pool).await;

        let result = engine.classify("Code", None, None);
        assert_eq!(result.category, "coding");
        assert_eq!(result.source, ClassificationSource::Rule);
        assert_eq!(result.rule_id.as_deref(), Some("test-app-1"));

        // Case-insensitive
        let result = engine.classify("code", None, None);
        assert_eq!(result.category, "coding");
    }

    #[tokio::test]
    async fn test_classify_url_contains_match() {
        let pool = setup_pool().await;
        sqlx::query("DELETE FROM productivity_tracking_rules")
            .execute(&pool)
            .await
            .unwrap();

        let rule_repo = TrackingRuleRepo::new(pool.clone());
        let now = Timestamp::now();
        let rule = TrackingRule {
            id: "test-url-1".to_string(),
            rule_type: RuleType::Url,
            match_field: "url".to_string(),
            match_pattern: "github.com".to_string(),
            match_mode: MatchMode::Contains,
            category: "coding".to_string(),
            session_type: IntelligenceSessionType::Focus,
            priority: 20,
            source: RuleSource::System,
            confidence: 1.0,
            hit_count: 0,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        rule_repo.create(&rule).await.unwrap();

        let engine = setup_engine(&pool).await;
        let result = engine.classify("Safari", Some("https://github.com/repo"), None);
        assert_eq!(result.category, "coding");
        assert_eq!(result.source, ClassificationSource::Rule);
    }

    #[tokio::test]
    async fn test_classify_regex_fallback() {
        let pool = setup_pool().await;
        sqlx::query("DELETE FROM productivity_tracking_rules")
            .execute(&pool)
            .await
            .unwrap();

        let rule_repo = TrackingRuleRepo::new(pool.clone());
        let now = Timestamp::now();
        let rule = TrackingRule {
            id: "test-regex-1".to_string(),
            rule_type: RuleType::Title,
            match_field: "window_title".to_string(),
            match_pattern: r".*\.rs\s".to_string(),
            match_mode: MatchMode::Regex,
            category: "coding".to_string(),
            session_type: IntelligenceSessionType::Focus,
            priority: 50,
            source: RuleSource::System,
            confidence: 0.8,
            hit_count: 0,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        rule_repo.create(&rule).await.unwrap();

        let engine = setup_engine(&pool).await;
        let result = engine.classify("SomeEditor", None, Some("main.rs - SomeEditor"));
        assert_eq!(result.category, "coding");
        assert_eq!(result.source, ClassificationSource::Rule);
    }

    #[tokio::test]
    async fn test_classify_priority_ordering() {
        let pool = setup_pool().await;
        sqlx::query("DELETE FROM productivity_tracking_rules")
            .execute(&pool)
            .await
            .unwrap();

        let rule_repo = TrackingRuleRepo::new(pool.clone());
        let now = Timestamp::now();
        let low_priority = TrackingRule {
            id: "prio-low".to_string(),
            rule_type: RuleType::Url,
            match_field: "url".to_string(),
            match_pattern: "slack.com".to_string(),
            match_mode: MatchMode::Contains,
            category: "communication".to_string(),
            session_type: IntelligenceSessionType::Meeting,
            priority: 100,
            source: RuleSource::System,
            confidence: 0.9,
            hit_count: 0,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        let high_priority = TrackingRule {
            id: "prio-high".to_string(),
            rule_type: RuleType::Url,
            match_field: "url".to_string(),
            match_pattern: "slack.com".to_string(),
            match_mode: MatchMode::Contains,
            category: "collab".to_string(),
            session_type: IntelligenceSessionType::Focus,
            priority: 5,
            source: RuleSource::User,
            confidence: 1.0,
            hit_count: 0,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        rule_repo.create(&low_priority).await.unwrap();
        rule_repo.create(&high_priority).await.unwrap();

        let engine = setup_engine(&pool).await;
        let result = engine.classify("Browser", Some("https://slack.com/channel"), None);
        assert_eq!(result.category, "collab");
        assert_eq!(result.rule_id.as_deref(), Some("prio-high"));
    }

    #[tokio::test]
    async fn test_classify_no_match_returns_default() {
        let pool = setup_pool().await;
        sqlx::query("DELETE FROM productivity_tracking_rules")
            .execute(&pool)
            .await
            .unwrap();

        let engine = setup_engine(&pool).await;
        let result = engine.classify("SomeUnknownApp", None, None);
        assert_eq!(result.source, ClassificationSource::Default);
        assert_eq!(result.category, "uncategorized");
        assert!(result.rule_id.is_none());
    }

    #[tokio::test]
    async fn test_reload_picks_up_new_rules() {
        let pool = setup_pool().await;
        sqlx::query("DELETE FROM productivity_tracking_rules")
            .execute(&pool)
            .await
            .unwrap();

        let mut engine = setup_engine(&pool).await;

        // No match initially
        let result = engine.classify("NewApp", None, None);
        assert_eq!(result.source, ClassificationSource::Default);

        // Evolve a new rule
        let rule = sample_rule("evolved-1", "NewApp", "design");
        let entry = RuleEvolutionEntry {
            id: "evo-1".to_string(),
            rule_id: "evolved-1".to_string(),
            action: "created".to_string(),
            old_confidence: None,
            new_confidence: Some(0.95),
            old_category: None,
            new_category: Some("design".to_string()),
            trigger_source: Some("test".to_string()),
            evidence_count: 1,
            created_at: jiff::Timestamp::now().to_string(),
        };
        engine.evolve_rule(&rule, &entry).await.unwrap();

        // Now it should match
        let result = engine.classify("NewApp", None, None);
        assert_eq!(result.category, "design");
        assert_eq!(result.source, ClassificationSource::Rule);
    }

    #[tokio::test]
    async fn test_privacy_filter_excludes_app() {
        let pool = setup_pool().await;
        let privacy_repo = PrivacyRuleRepo::new(pool.clone());
        privacy_repo
            .create(&PrivacyRule {
                id: "priv-1".to_string(),
                rule_type: "exclude_app".to_string(),
                pattern: "1Password".to_string(),
                match_mode: MatchMode::Exact,
                created_at: Timestamp::now(),
            })
            .await
            .unwrap();

        let engine = setup_engine(&pool).await;

        let result = engine.apply_privacy("1Password", Some("Vault"), None);
        assert!(result.excluded);
        assert!(result.title.is_none());

        // Case-insensitive
        let result = engine.apply_privacy("1password", Some("Vault"), None);
        assert!(result.excluded);
    }

    #[tokio::test]
    async fn test_privacy_filter_redacts_title() {
        let pool = setup_pool().await;
        let privacy_repo = PrivacyRuleRepo::new(pool.clone());
        privacy_repo
            .create(&PrivacyRule {
                id: "priv-redact-1".to_string(),
                rule_type: "redact_title".to_string(),
                pattern: "banking".to_string(),
                match_mode: MatchMode::Contains,
                created_at: Timestamp::now(),
            })
            .await
            .unwrap();

        let engine = setup_engine(&pool).await;

        // Title containing "banking" should be redacted
        let result = engine.apply_privacy("Safari", Some("Online Banking Portal"), None);
        assert!(!result.excluded);
        assert!(result.title.is_none());

        // Non-banking title should pass through
        let result = engine.apply_privacy("Safari", Some("GitHub"), None);
        assert!(!result.excluded);
        assert_eq!(result.title.as_deref(), Some("GitHub"));
    }
}
