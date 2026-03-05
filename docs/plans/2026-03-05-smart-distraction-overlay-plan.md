# Smart Distraction Overlay Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a real-time floating overlay window that intercedes when users switch to distracting apps during focus sessions, with hybrid heuristic+LLM content classification and persistent learning.

**Architecture:** `DistractionInterceptor` sits between the existing `DistractionAlert` mpsc channel and the UI. It checks session whitelists, learned rules, and heuristics before deciding to show an overlay. Ambiguous cases trigger an async LLM classification. User choices feed back into a learning system that reduces false positives over time.

**Tech Stack:** Rust (feature-productivity crate), Tauri v2 webview windows, React + Tailwind v4 (desktop-ui), SQLite (learned rules persistence).

---

## Task 1: Add Config Fields to FocusConfig

**Files:**
- Modify: `crates/config/src/schema/productivity.rs:36-49` (FocusConfig struct)
- Modify: `crates/config/src/schema/productivity.rs:132-141` (Default impl)

**Step 1: Add new fields to FocusConfig**

In `crates/config/src/schema/productivity.rs`, add fields after `soft_block_enabled`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusConfig {
    #[serde(default = "default_focus_duration")]
    pub default_duration_mins: u64,
    #[serde(default = "default_break_interval")]
    pub break_interval_mins: u64,
    #[serde(default = "default_break_duration")]
    pub break_duration_mins: u64,
    #[serde(default = "default_max_daily_focus")]
    pub max_daily_focus_hours: u64,
    #[serde(default = "default_true")]
    pub soft_block_enabled: bool,
    #[serde(default = "default_soft_block_cooldown")]
    pub soft_block_cooldown_secs: u64,
    #[serde(default = "default_temp_pass_mins")]
    pub soft_block_temp_pass_mins: u64,
    #[serde(default = "default_true")]
    pub soft_block_llm_enabled: bool,
    #[serde(default = "default_llm_timeout")]
    pub soft_block_llm_timeout_ms: u64,
    #[serde(default = "default_learned_rule_threshold")]
    pub learned_rule_threshold: u64,
}
```

**Step 2: Add default functions and update Default impl**

```rust
fn default_soft_block_cooldown() -> u64 { 60 }
fn default_temp_pass_mins() -> u64 { 5 }
fn default_llm_timeout() -> u64 { 3000 }
fn default_learned_rule_threshold() -> u64 { 3 }

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            default_duration_mins: default_focus_duration(),
            break_interval_mins: default_break_interval(),
            break_duration_mins: default_break_duration(),
            max_daily_focus_hours: default_max_daily_focus(),
            soft_block_enabled: true,
            soft_block_cooldown_secs: default_soft_block_cooldown(),
            soft_block_temp_pass_mins: default_temp_pass_mins(),
            soft_block_llm_enabled: true,
            soft_block_llm_timeout_ms: default_llm_timeout(),
            learned_rule_threshold: default_learned_rule_threshold(),
        }
    }
}
```

**Step 3: Verify compilation**

Run: `cargo build -p config`
Expected: Compiles with 0 errors. Existing tests pass since all new fields have defaults.

**Step 4: Run existing config tests**

Run: `cargo nextest run -p config`
Expected: All tests pass (serde defaults cover new fields).

**Step 5: Commit**

```bash
git add crates/config/src/schema/productivity.rs
git commit -m "feat(config): add soft block config fields to FocusConfig"
```

---

## Task 2: Migration + Learned Rules Repo

**Files:**
- Create: `crates/feature-productivity/migrations/004_distraction_learned_rules.sql`
- Create: `crates/feature-productivity/src/repos/learned_rule.rs`
- Modify: `crates/feature-productivity/src/repos/mod.rs` (add repo + aggregate field)
- Modify: `crates/feature-productivity/src/lib.rs:47-67` (register migration v4)

**Step 1: Write the migration**

Create `crates/feature-productivity/migrations/004_distraction_learned_rules.sql`:

```sql
-- Persistent learned rules for distraction classification.
-- Tracks user overrides so the system stops nagging about known work-related content.
CREATE TABLE IF NOT EXISTS distraction_learned_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern TEXT NOT NULL,
    pattern_type TEXT NOT NULL CHECK (pattern_type IN ('title_keyword', 'app_name', 'url_pattern')),
    classification TEXT NOT NULL CHECK (classification IN ('educational', 'work_research')),
    confidence REAL NOT NULL DEFAULT 0.5,
    hit_count INTEGER NOT NULL DEFAULT 1,
    last_used_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_learned_rules_pattern ON distraction_learned_rules (pattern, pattern_type);
```

**Step 2: Write the repo with tests**

Create `crates/feature-productivity/src/repos/learned_rule.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LearnedRule {
    pub id: Option<i64>,
    pub pattern: String,
    pub pattern_type: String,
    pub classification: String,
    pub confidence: f64,
    pub hit_count: i64,
    pub last_used_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LearnedRuleRepo {
    pool: SqlitePool,
}

impl LearnedRuleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> common::Result<Vec<LearnedRule>> {
        let rules = sqlx::query_as::<_, LearnedRule>(
            "SELECT * FROM distraction_learned_rules ORDER BY hit_count DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::StorageError::Query(e.to_string()))?;
        Ok(rules)
    }

    /// Find a rule matching the given pattern and type.
    pub async fn find_by_pattern(
        &self,
        pattern: &str,
        pattern_type: &str,
    ) -> common::Result<Option<LearnedRule>> {
        let rule = sqlx::query_as::<_, LearnedRule>(
            "SELECT * FROM distraction_learned_rules WHERE pattern = ? AND pattern_type = ?",
        )
        .bind(pattern)
        .bind(pattern_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::StorageError::Query(e.to_string()))?;
        Ok(rule)
    }

    /// Insert a new learned rule.
    pub async fn insert(&self, rule: &LearnedRule) -> common::Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO distraction_learned_rules (pattern, pattern_type, classification, confidence, hit_count, last_used_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(&rule.pattern)
        .bind(&rule.pattern_type)
        .bind(&rule.classification)
        .bind(rule.confidence)
        .bind(rule.hit_count)
        .bind(rule.last_used_at)
        .bind(rule.created_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::StorageError::Query(e.to_string()))?;
        Ok(id)
    }

    /// Increment hit_count and update confidence + last_used_at for an existing rule.
    pub async fn record_hit(&self, id: i64) -> common::Result<()> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE distraction_learned_rules SET hit_count = hit_count + 1, confidence = MIN(1.0, confidence + 0.1), last_used_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::StorageError::Query(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> common::Result<()> {
        sqlx::query("DELETE FROM distraction_learned_rules WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::StorageError::Query(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    #[tokio::test]
    async fn insert_and_find() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Utc::now();
        let rule = LearnedRule {
            id: None,
            pattern: "react tutorial".into(),
            pattern_type: "title_keyword".into(),
            classification: "educational".into(),
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        let id = repo.insert(&rule).await.unwrap();
        assert!(id > 0);

        let found = repo.find_by_pattern("react tutorial", "title_keyword").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().classification, "educational");
    }

    #[tokio::test]
    async fn record_hit_increments() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Utc::now();
        let rule = LearnedRule {
            id: None,
            pattern: "rust docs".into(),
            pattern_type: "title_keyword".into(),
            classification: "work_research".into(),
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        let id = repo.insert(&rule).await.unwrap();
        repo.record_hit(id).await.unwrap();

        let found = repo.find_by_pattern("rust docs", "title_keyword").await.unwrap().unwrap();
        assert_eq!(found.hit_count, 2);
        assert!((found.confidence - 0.6).abs() < 0.01);
    }

    #[tokio::test]
    async fn delete_removes_rule() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Utc::now();
        let rule = LearnedRule {
            id: None,
            pattern: "temp".into(),
            pattern_type: "app_name".into(),
            classification: "educational".into(),
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        let id = repo.insert(&rule).await.unwrap();
        repo.delete(id).await.unwrap();
        assert!(repo.find_by_pattern("temp", "app_name").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_all_ordered_by_hit_count() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Utc::now();

        let low = LearnedRule {
            id: None, pattern: "a".into(), pattern_type: "title_keyword".into(),
            classification: "educational".into(), confidence: 0.5, hit_count: 1,
            last_used_at: now, created_at: now,
        };
        let high = LearnedRule {
            id: None, pattern: "b".into(), pattern_type: "title_keyword".into(),
            classification: "educational".into(), confidence: 0.9, hit_count: 10,
            last_used_at: now, created_at: now,
        };
        repo.insert(&low).await.unwrap();
        repo.insert(&high).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].pattern, "b"); // higher hit_count first
    }
}
```

**Step 3: Wire repo into ProductivityRepos**

In `crates/feature-productivity/src/repos/mod.rs`, add:
- `pub mod learned_rule;`
- `pub use learned_rule::LearnedRuleRepo;`
- Add field `pub learned_rules: LearnedRuleRepo` to `ProductivityRepos`
- Initialize it in `ProductivityRepos::new()`

**Step 4: Register migration v4**

In `crates/feature-productivity/src/lib.rs`, add to `migrations_static()`:

```rust
FeatureMigration {
    feature_name: "productivity".to_string(),
    version: 4,
    description: "Add distraction learned rules table".to_string(),
    sql: include_str!("../migrations/004_distraction_learned_rules.sql").to_string(),
},
```

**Step 5: Run tests**

Run: `cargo nextest run -p feature-productivity`
Expected: All existing tests + new repo tests pass.

**Step 6: Commit**

```bash
git add crates/feature-productivity/migrations/004_distraction_learned_rules.sql \
        crates/feature-productivity/src/repos/learned_rule.rs \
        crates/feature-productivity/src/repos/mod.rs \
        crates/feature-productivity/src/lib.rs
git commit -m "feat(productivity): add learned rules migration and repo"
```

---

## Task 3: Heuristic Classifier

**Files:**
- Create: `crates/feature-productivity/src/distraction/heuristics.rs`
- Create: `crates/feature-productivity/src/distraction/mod.rs`
- Modify: `crates/feature-productivity/src/lib.rs` (add `pub mod distraction;`)

**Step 1: Create the distraction module**

Create `crates/feature-productivity/src/distraction/mod.rs`:

```rust
pub mod heuristics;
```

Add `pub mod distraction;` to `crates/feature-productivity/src/lib.rs`.

**Step 2: Write the heuristic classifier with tests**

Create `crates/feature-productivity/src/distraction/heuristics.rs`:

```rust
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

/// Apps that are always distracting regardless of content.
const ALWAYS_DISTRACTING_APPS: &[&str] = &[
    "Netflix", "TikTok", "Instagram", "Twitch", "Discord",
];

/// Window title keywords that signal definite distraction.
const DISTRACTING_TITLE_KEYWORDS: &[&str] = &[
    "facebook", "hacker news", "tiktok", "instagram", "twitch.tv", "netflix",
];

/// Window title keywords that signal productive content.
const PRODUCTIVE_TITLE_KEYWORDS: &[&str] = &[
    "stack overflow", "stackoverflow", "mdn web docs", "docs.rs",
    "github issues", "github pull", "crates.io", "npm", "pypi",
    "developer.apple.com", "developer.mozilla", "cppreference",
    "arxiv", "google scholar", "coursera", "udemy", "edx",
];

/// Apps whose content is inherently ambiguous (same app, different content).
const AMBIGUOUS_APPS: &[&str] = &[
    "YouTube", "Reddit",
];

/// Title keywords that hint at educational content within ambiguous apps.
const EDUCATIONAL_TITLE_HINTS: &[&str] = &[
    "tutorial", "course", "lecture", "conference", "talk",
    "how to", "guide", "documentation", "docs", "explained",
    "deep dive", "walkthrough", "workshop", "lesson",
];

/// Classify a distraction alert using fast heuristics.
///
/// `app_name`: The frontmost application name (e.g. "Google Chrome", "YouTube").
/// `window_title`: The window title, often contains site name for browsers.
pub fn classify(app_name: &str, window_title: Option<&str>) -> HeuristicVerdict {
    let app_lower = app_name.to_lowercase();
    let title_lower = window_title.map(|t| t.to_lowercase()).unwrap_or_default();

    // 1. Check always-distracting apps (by app name).
    if ALWAYS_DISTRACTING_APPS.iter().any(|a| app_lower == a.to_lowercase()) {
        return HeuristicVerdict::ConfidentDistracting;
    }

    // 2. Check productive title keywords — takes priority over distracting.
    if PRODUCTIVE_TITLE_KEYWORDS.iter().any(|k| title_lower.contains(k)) {
        return HeuristicVerdict::ConfidentProductive;
    }

    // 3. Check distracting title keywords.
    if DISTRACTING_TITLE_KEYWORDS.iter().any(|k| title_lower.contains(k)) {
        return HeuristicVerdict::ConfidentDistracting;
    }

    // 4. Check ambiguous apps — these need LLM classification.
    if AMBIGUOUS_APPS.iter().any(|a| app_lower.contains(&a.to_lowercase()) || title_lower.contains(&a.to_lowercase())) {
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
        assert_eq!(classify("Netflix", None), HeuristicVerdict::ConfidentDistracting);
        assert_eq!(classify("TikTok", None), HeuristicVerdict::ConfidentDistracting);
        assert_eq!(classify("Instagram", Some("Feed")), HeuristicVerdict::ConfidentDistracting);
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
```

**Step 3: Verify tests pass**

Run: `cargo nextest run -p feature-productivity -E 'test(heuristic)'`
Expected: All heuristic tests pass.

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/distraction/
git commit -m "feat(productivity): add heuristic distraction classifier"
```

---

## Task 4: DistractionInterceptor Core

**Files:**
- Create: `crates/feature-productivity/src/distraction/interceptor.rs`
- Modify: `crates/feature-productivity/src/distraction/mod.rs` (add module)

This is the brain that sits between `DistractionAlert` and the overlay. It manages session whitelist, temp passes, learned rule lookups, and heuristic classification.

**Step 1: Write the interceptor**

Create `crates/feature-productivity/src/distraction/interceptor.rs`:

```rust
//! DistractionInterceptor — decides whether to show the overlay for a distraction alert.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use chrono::{DateTime, Utc};

use crate::config::FocusConfig;
use crate::repos::learned_rule::LearnedRuleRepo;
use super::heuristics::{self, HeuristicVerdict};

/// The interceptor's decision for a given distraction alert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterceptDecision {
    /// Skip — content is whitelisted or classified as productive.
    Allow { reason: String },
    /// Show the overlay immediately — content is definitely distracting.
    ShowOverlay { needs_llm: bool },
}

/// Payload sent to the overlay window.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterventionPayload {
    pub app_name: String,
    pub window_title: Option<String>,
    pub session_id: String,
    pub needs_llm: bool,
    pub heuristic_verdict: String,
}

pub struct DistractionInterceptor {
    config: FocusConfig,
    learned_rules_repo: LearnedRuleRepo,
    /// Titles whitelisted for the current focus session.
    session_whitelist: HashSet<String>,
    /// Temporary passes: pattern -> expiry instant.
    temp_passes: HashMap<String, Instant>,
}

impl DistractionInterceptor {
    pub fn new(config: FocusConfig, learned_rules_repo: LearnedRuleRepo) -> Self {
        Self {
            config,
            learned_rules_repo,
            session_whitelist: HashSet::new(),
            temp_passes: HashMap::new(),
        }
    }

    /// Clear all session state (call when focus session ends).
    pub fn reset_session(&mut self) {
        self.session_whitelist.clear();
        self.temp_passes.clear();
    }

    /// Add a title pattern to the session whitelist ("This is work-related").
    pub fn whitelist_for_session(&mut self, pattern: &str) {
        self.session_whitelist.insert(pattern.to_lowercase());
    }

    /// Grant a temporary pass for the given pattern.
    pub fn grant_temp_pass(&mut self, pattern: &str) {
        let expiry = Instant::now()
            + std::time::Duration::from_secs(self.config.soft_block_temp_pass_mins * 60);
        self.temp_passes.insert(pattern.to_lowercase(), expiry);
    }

    /// Evaluate a distraction alert and decide what to do.
    pub async fn evaluate(
        &mut self,
        app_name: &str,
        window_title: Option<&str>,
    ) -> InterceptDecision {
        if !self.config.soft_block_enabled {
            return InterceptDecision::Allow {
                reason: "soft_block_enabled is false".into(),
            };
        }

        let key = Self::make_key(app_name, window_title);

        // 1. Check session whitelist.
        if self.session_whitelist.contains(&key) {
            return InterceptDecision::Allow {
                reason: "session whitelist".into(),
            };
        }

        // 2. Check temp passes (and prune expired).
        self.temp_passes.retain(|_, expiry| *expiry > Instant::now());
        if self.temp_passes.contains_key(&key) {
            return InterceptDecision::Allow {
                reason: "temporary pass active".into(),
            };
        }

        // 3. Check persistent learned rules.
        if let Ok(Some(rule)) = self
            .learned_rules_repo
            .find_by_pattern(&key, "title_keyword")
            .await
        {
            if rule.hit_count >= self.config.learned_rule_threshold as i64 {
                // Auto-allow — this pattern has been confirmed enough times.
                let _ = self.learned_rules_repo.record_hit(rule.id.unwrap_or(0)).await;
                return InterceptDecision::Allow {
                    reason: format!("learned rule: {} ({}x)", rule.classification, rule.hit_count),
                };
            }
        }

        // 4. Run heuristics.
        let verdict = heuristics::classify(app_name, window_title);
        match verdict {
            HeuristicVerdict::ConfidentProductive => InterceptDecision::Allow {
                reason: "heuristic: confident productive".into(),
            },
            HeuristicVerdict::ConfidentDistracting => InterceptDecision::ShowOverlay {
                needs_llm: false,
            },
            HeuristicVerdict::Ambiguous => InterceptDecision::ShowOverlay {
                needs_llm: self.config.soft_block_llm_enabled,
            },
        }
    }

    /// Normalize app_name + window_title into a lookup key.
    fn make_key(app_name: &str, window_title: Option<&str>) -> String {
        match window_title {
            Some(title) => title.to_lowercase(),
            None => app_name.to_lowercase(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup() -> DistractionInterceptor {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        let repo = LearnedRuleRepo::new(inner);
        DistractionInterceptor::new(FocusConfig::default(), repo)
    }

    #[tokio::test]
    async fn disabled_soft_block_allows_all() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        let repo = LearnedRuleRepo::new(inner);
        let mut config = FocusConfig::default();
        config.soft_block_enabled = false;
        let mut interceptor = DistractionInterceptor::new(config, repo);

        let decision = interceptor.evaluate("Netflix", None).await;
        assert!(matches!(decision, InterceptDecision::Allow { .. }));
    }

    #[tokio::test]
    async fn session_whitelist_allows() {
        let mut interceptor = setup().await;
        interceptor.whitelist_for_session("funny cats - youtube");

        let decision = interceptor
            .evaluate("Chrome", Some("Funny Cats - YouTube"))
            .await;
        assert!(matches!(decision, InterceptDecision::Allow { .. }));
    }

    #[tokio::test]
    async fn temp_pass_allows_then_expires() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        let repo = LearnedRuleRepo::new(inner);
        let mut config = FocusConfig::default();
        config.soft_block_temp_pass_mins = 0; // 0 minutes = expires immediately
        let mut interceptor = DistractionInterceptor::new(config, repo);

        interceptor.grant_temp_pass("reddit");
        // Immediately expired since temp_pass_mins = 0
        std::thread::sleep(std::time::Duration::from_millis(10));
        let decision = interceptor.evaluate("Reddit", None).await;
        // Should NOT be allowed (pass expired)
        assert!(matches!(decision, InterceptDecision::ShowOverlay { .. }));
    }

    #[tokio::test]
    async fn netflix_shows_overlay_no_llm() {
        let mut interceptor = setup().await;
        let decision = interceptor.evaluate("Netflix", None).await;
        assert_eq!(decision, InterceptDecision::ShowOverlay { needs_llm: false });
    }

    #[tokio::test]
    async fn youtube_shows_overlay_with_llm() {
        let mut interceptor = setup().await;
        let decision = interceptor
            .evaluate("Chrome", Some("Some Video - YouTube"))
            .await;
        assert_eq!(decision, InterceptDecision::ShowOverlay { needs_llm: true });
    }

    #[tokio::test]
    async fn stackoverflow_in_title_allows() {
        let mut interceptor = setup().await;
        let decision = interceptor
            .evaluate("Chrome", Some("Rust lifetime - Stack Overflow"))
            .await;
        assert!(matches!(decision, InterceptDecision::Allow { .. }));
    }

    #[tokio::test]
    async fn reset_session_clears_state() {
        let mut interceptor = setup().await;
        interceptor.whitelist_for_session("youtube");
        interceptor.reset_session();
        let decision = interceptor
            .evaluate("Chrome", Some("cat video - YouTube"))
            .await;
        assert!(matches!(decision, InterceptDecision::ShowOverlay { .. }));
    }
}
```

**Step 2: Update distraction/mod.rs**

```rust
pub mod heuristics;
pub mod interceptor;
```

**Step 3: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(interceptor)' -E 'test(heuristic)'`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/distraction/
git commit -m "feat(productivity): add DistractionInterceptor with session whitelist and temp passes"
```

---

## Task 5: DistractionClassifierHandler Trait (Dependency Inversion)

**Files:**
- Create: `crates/feature-productivity/src/distraction/classifier.rs`
- Modify: `crates/feature-productivity/src/distraction/mod.rs`

This defines the trait that the `desktop` crate will implement using the LLM provider.

**Step 1: Write the classifier trait**

Create `crates/feature-productivity/src/distraction/classifier.rs`:

```rust
//! Trait for async content classification (LLM-backed).

use async_trait::async_trait;

/// Classification result from the LLM.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentClassification {
    Educational,
    WorkResearch,
    Entertainment,
    SocialMedia,
    Unknown,
}

impl std::fmt::Display for ContentClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Educational => write!(f, "Likely educational content"),
            Self::WorkResearch => write!(f, "Likely work-related research"),
            Self::Entertainment => write!(f, "Entertainment detected"),
            Self::SocialMedia => write!(f, "Social media detected"),
            Self::Unknown => write!(f, "Unable to classify"),
        }
    }
}

#[async_trait]
pub trait DistractionClassifierHandler: Send + Sync {
    /// Classify the content at the given window title.
    /// Should complete within the configured timeout.
    async fn classify(
        &self,
        app_name: &str,
        window_title: &str,
    ) -> common::Result<ContentClassification>;
}
```

**Step 2: Update distraction/mod.rs**

```rust
pub mod classifier;
pub mod heuristics;
pub mod interceptor;

pub use classifier::{ContentClassification, DistractionClassifierHandler};
pub use heuristics::HeuristicVerdict;
pub use interceptor::{DistractionInterceptor, InterceptDecision, InterventionPayload};
```

**Step 3: Verify compilation**

Run: `cargo build -p feature-productivity`
Expected: Compiles cleanly.

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/distraction/
git commit -m "feat(productivity): add DistractionClassifierHandler trait for LLM classification"
```

---

## Task 6: Tauri Events + IPC Commands for Overlay

**Files:**
- Modify: `crates/desktop-shared/src/events.rs` (add new event constants + payload)
- Modify: `crates/desktop-shared/src/commands.rs` (add response types)
- Create: `crates/desktop/src/commands/distraction.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add `pub mod distraction;`)
- Modify: `crates/desktop/src/main.rs` (register commands + overlay window config)

**Step 1: Add events and payloads**

In `crates/desktop-shared/src/events.rs`, add after the existing productivity events:

```rust
pub const DISTRACTION_INTERVENTION: &str = "distraction:intervention";
pub const DISTRACTION_VERDICT: &str = "distraction:verdict";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterventionPayload {
    pub app_name: String,
    pub window_title: Option<String>,
    pub session_id: String,
    pub needs_llm: bool,
    pub heuristic_verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerdictPayload {
    pub classification: String,
    pub display_text: String,
}
```

**Step 2: Add command response types**

In `crates/desktop-shared/src/commands.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedRuleResponse {
    pub id: i64,
    pub pattern: String,
    pub pattern_type: String,
    pub classification: String,
    pub confidence: f64,
    pub hit_count: i64,
    pub last_used_at: String,
    pub created_at: String,
}
```

**Step 3: Write the distraction IPC commands**

Create `crates/desktop/src/commands/distraction.rs`:

```rust
//! Distraction overlay IPC commands.

use std::sync::Arc;
use desktop_shared::commands::LearnedRuleResponse;
use desktop_shared::errors::ApiError;
use tauri::State;
use crate::app_core::AppCore;

fn map_err(e: common::KlyntbotError) -> ApiError {
    ApiError::new("DISTRACTION_ERROR", e.to_string())
}

#[tauri::command]
pub async fn distraction_dismiss(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
    app_name: String,
) -> Result<(), ApiError> {
    let focus_mgr = state.focus_manager()?;
    focus_mgr.record_distraction(&app_name).await.map_err(map_err)?;
    Ok(())
}

#[tauri::command]
pub async fn distraction_allow_temp(
    state: State<'_, Arc<AppCore>>,
    pattern: String,
) -> Result<(), ApiError> {
    let interceptor = state.distraction_interceptor()?;
    let mut guard = interceptor.lock().await;
    guard.grant_temp_pass(&pattern);
    Ok(())
}

#[tauri::command]
pub async fn distraction_allow_session(
    state: State<'_, Arc<AppCore>>,
    pattern: String,
    classification: String,
) -> Result<(), ApiError> {
    let interceptor = state.distraction_interceptor()?;
    let mut guard = interceptor.lock().await;
    guard.whitelist_for_session(&pattern);
    drop(guard);

    // Record as learning candidate
    let repos = state.productivity_repos()?;
    let now = chrono::Utc::now();
    let existing = repos
        .learned_rules
        .find_by_pattern(&pattern.to_lowercase(), "title_keyword")
        .await
        .map_err(map_err)?;

    if let Some(rule) = existing {
        repos.learned_rules.record_hit(rule.id.unwrap_or(0)).await.map_err(map_err)?;
    } else {
        use crate::app_core::LearnedRule;
        let rule = LearnedRule {
            id: None,
            pattern: pattern.to_lowercase(),
            pattern_type: "title_keyword".into(),
            classification,
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        repos.learned_rules.insert(&rule).await.map_err(map_err)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn distraction_learned_rules(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<LearnedRuleResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let rules = repos.learned_rules.list_all().await.map_err(map_err)?;
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

#[tauri::command]
pub async fn distraction_delete_rule(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.learned_rules.delete(id).await.map_err(map_err)?;
    Ok(())
}
```

**Step 4: Wire into AppCore**

In `crates/desktop/src/app_core.rs`, add:
- A `distraction_interceptor: Option<Arc<tokio::sync::Mutex<DistractionInterceptor>>>` field to `AppCore`
- A `pub fn distraction_interceptor(&self) -> Result<Arc<...>, ApiError>` accessor
- Initialize the interceptor during `AppCore::init()` alongside the focus manager
- Use type alias: `pub use feature_productivity::repos::learned_rule::LearnedRule;`

**Step 5: Register commands in main.rs**

In `crates/desktop/src/main.rs`:
- Add `pub mod distraction;` to `commands/mod.rs`
- Add the 5 new commands to `invoke_handler(tauri::generate_handler![...])` after the existing productivity commands

**Step 6: Add overlay window to tauri.conf.json**

Add to the `app.windows` array in `crates/desktop/tauri.conf.json`:

```json
{
    "label": "distraction-overlay",
    "url": "/#/distraction-overlay",
    "title": "",
    "width": 420,
    "height": 280,
    "resizable": false,
    "decorations": false,
    "visible": false,
    "transparent": true,
    "shadow": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "center": true,
    "focus": true
}
```

**Step 7: Verify compilation**

Run: `cargo build -p desktop --lib` (skip binary since it needs AppKit)
Expected: Compiles with 0 errors. (Note: full binary build requires macOS AppKit — verify with `cargo check -p desktop` if needed.)

**Step 8: Commit**

```bash
git add crates/desktop-shared/src/events.rs \
        crates/desktop-shared/src/commands.rs \
        crates/desktop/src/commands/distraction.rs \
        crates/desktop/src/commands/mod.rs \
        crates/desktop/src/app_core.rs \
        crates/desktop/src/main.rs \
        crates/desktop/tauri.conf.json
git commit -m "feat(desktop): add distraction overlay IPC commands and window config"
```

---

## Task 7: Wire DistractionInterceptor into Alert Pipeline

**Files:**
- Modify: `crates/desktop/src/app_core.rs` (`spawn_productivity_events` method)

Currently `spawn_productivity_events` just forwards `DistractionAlert` directly as Tauri events. We need to route alerts through the `DistractionInterceptor` first.

**Step 1: Update spawn_productivity_events**

Replace the existing distraction alert forwarder with one that runs through the interceptor:

```rust
// In spawn_productivity_events, replace the distraction forwarder:
Self::spawn_channel_forwarder(
    distraction_rx,
    app_handle,
    shutdown_token,
    {
        let interceptor = Arc::clone(&interceptor_arc);
        move |handle, alert: feature_productivity::tracker::DistractionAlert| {
            let interceptor = Arc::clone(&interceptor);
            let handle = handle.clone();
            let app_name = alert.app_name.clone();
            let session_id = alert.session_id.clone();

            // Run interceptor evaluation async
            tokio::spawn(async move {
                let window_title = None; // Title comes from the alert's app_name context
                let mut guard = interceptor.lock().await;
                let decision = guard.evaluate(&app_name, window_title).await;
                drop(guard);

                match decision {
                    feature_productivity::distraction::InterceptDecision::Allow { reason } => {
                        tracing::debug!("Distraction allowed: {reason}");
                    }
                    feature_productivity::distraction::InterceptDecision::ShowOverlay { needs_llm } => {
                        // Show the overlay window
                        if let Some(window) = handle.get_webview_window("distraction-overlay") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        // Emit intervention event
                        let payload = desktop_shared::events::InterventionPayload {
                            app_name,
                            window_title: None,
                            session_id,
                            needs_llm,
                            heuristic_verdict: if needs_llm { "ambiguous" } else { "distracting" }.into(),
                        };
                        if let Err(e) = handle.emit(
                            desktop_shared::events::DISTRACTION_INTERVENTION,
                            payload,
                        ) {
                            tracing::warn!("failed to emit distraction intervention: {e}");
                        }
                    }
                }
            });
        }
    },
);
```

**Important:** The `DistractionAlert` currently only carries `app_name` and `session_id`. To support window title in the interceptor, we need to extend `DistractionAlert` in the tracker to include the window title. Modify `crates/feature-productivity/src/tracker/mod.rs`:

```rust
pub struct DistractionAlert {
    pub app_name: String,
    pub window_title: Option<String>,  // ADD THIS
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
}
```

And update the tracker's `try_send` call (around line 185) to include the window title:

```rust
if let Err(e) = sender.try_send(DistractionAlert {
    app_name: info.app_name.clone(),
    window_title: info.window_title.clone(),  // ADD THIS
    session_id,
    timestamp: now,
}) {
```

**Step 2: Verify compilation**

Run: `cargo check -p desktop`
Expected: Compiles (or check errors and fix any import issues).

**Step 3: Commit**

```bash
git add crates/feature-productivity/src/tracker/mod.rs \
        crates/desktop/src/app_core.rs
git commit -m "feat(desktop): route distraction alerts through interceptor before overlay"
```

---

## Task 8: LLM Classifier Implementation

**Files:**
- Create: `crates/desktop/src/distraction_classifier.rs`
- Modify: `crates/desktop/src/app_core.rs` (integrate classifier)

**Step 1: Implement the classifier**

Create `crates/desktop/src/distraction_classifier.rs`:

```rust
//! LLM-backed distraction content classifier.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use feature_productivity::distraction::{ContentClassification, DistractionClassifierHandler};
use providers::LlmProvider;

pub struct LlmDistractionClassifier {
    provider: Arc<dyn LlmProvider>,
    timeout: Duration,
}

impl LlmDistractionClassifier {
    pub fn new(provider: Arc<dyn LlmProvider>, timeout_ms: u64) -> Self {
        Self {
            provider,
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl DistractionClassifierHandler for LlmDistractionClassifier {
    async fn classify(
        &self,
        app_name: &str,
        window_title: &str,
    ) -> common::Result<ContentClassification> {
        let prompt = format!(
            "Classify this window activity as one of: educational, work_research, entertainment, social_media.\n\
             App: {app_name}\n\
             Window title: {window_title}\n\n\
             Respond with ONLY one of: educational, work_research, entertainment, social_media"
        );

        let messages = vec![common::ChatMessage {
            role: common::MessageRole::User,
            content: prompt,
        }];

        let result = tokio::time::timeout(
            self.timeout,
            self.provider.chat(&messages, None, None, None),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                let text = response.content.trim().to_lowercase();
                Ok(match text.as_str() {
                    "educational" => ContentClassification::Educational,
                    "work_research" => ContentClassification::WorkResearch,
                    "entertainment" => ContentClassification::Entertainment,
                    "social_media" => ContentClassification::SocialMedia,
                    _ => ContentClassification::Unknown,
                })
            }
            Ok(Err(_)) | Err(_) => Ok(ContentClassification::Unknown),
        }
    }
}
```

**Note:** The exact `ChatMessage` and `LlmProvider` types depend on the `providers` and `common` crate APIs. The implementor should check `crates/providers/src/lib.rs` and `crates/common/src/lib.rs` for the correct types and adjust imports. The core logic (prompt → parse single-word response) stays the same.

**Step 2: Wire into alert pipeline for async LLM**

In the overlay-showing branch of `spawn_productivity_events` (Task 7), after emitting the intervention event, if `needs_llm` is true, spawn an async LLM call:

```rust
if needs_llm {
    if let Some(classifier) = classifier_opt.as_ref() {
        let classifier = Arc::clone(classifier);
        let handle2 = handle.clone();
        let title = window_title_clone.unwrap_or_default();
        let app = app_name_clone;
        tokio::spawn(async move {
            let result = classifier.classify(&app, &title).await;
            let classification = result.unwrap_or(ContentClassification::Unknown);
            let payload = desktop_shared::events::VerdictPayload {
                classification: format!("{classification:?}").to_lowercase(),
                display_text: classification.to_string(),
            };
            let _ = handle2.emit(desktop_shared::events::DISTRACTION_VERDICT, payload);
        });
    }
}
```

**Step 3: Verify compilation**

Run: `cargo check -p desktop`
Expected: Compiles.

**Step 4: Commit**

```bash
git add crates/desktop/src/distraction_classifier.rs \
        crates/desktop/src/app_core.rs
git commit -m "feat(desktop): add LLM distraction classifier with timeout"
```

---

## Task 9: Overlay UI Component

**Files:**
- Create: `desktop-ui/src/components/distraction/DistractionOverlay.tsx`
- Modify: `desktop-ui/src/App.tsx` (add route)

**Step 1: Create the overlay component**

Create `desktop-ui/src/components/distraction/DistractionOverlay.tsx`:

```tsx
import { useState, useEffect } from 'react';
import { useEvent } from '../../hooks/useEvent';
import { ipc } from '../../hooks/useIpc';

interface InterventionPayload {
  appName: string;
  windowTitle: string | null;
  sessionId: string;
  needsLlm: boolean;
  heuristicVerdict: string;
}

interface VerdictPayload {
  classification: string;
  displayText: string;
}

export function DistractionOverlay() {
  const [intervention, setIntervention] = useState<InterventionPayload | null>(null);
  const [verdict, setVerdict] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEvent<InterventionPayload>('distraction:intervention', (payload) => {
    setIntervention(payload);
    setVerdict(null);
    if (payload.needsLlm) {
      setLoading(true);
    }
  });

  useEvent<VerdictPayload>('distraction:verdict', (payload) => {
    setVerdict(payload.displayText);
    setLoading(false);
  });

  if (!intervention) {
    return (
      <div className="w-screen h-screen flex items-center justify-center">
        <div className="text-dim text-sm">Waiting for intervention...</div>
      </div>
    );
  }

  const titleExcerpt = intervention.windowTitle
    ? intervention.windowTitle.length > 60
      ? intervention.windowTitle.slice(0, 60) + '...'
      : intervention.windowTitle
    : null;

  const pattern = intervention.windowTitle?.toLowerCase() ?? intervention.appName.toLowerCase();

  const handleDismiss = async () => {
    await ipc('distraction_dismiss', {
      sessionId: intervention.sessionId,
      appName: intervention.appName,
    });
    await hideWindow();
  };

  const handleAllowTemp = async () => {
    await ipc('distraction_allow_temp', { pattern });
    await hideWindow();
  };

  const handleAllowSession = async () => {
    await ipc('distraction_allow_session', {
      pattern,
      classification: verdict?.includes('educational') ? 'educational' : 'work_research',
    });
    await hideWindow();
  };

  const hideWindow = async () => {
    setIntervention(null);
    setVerdict(null);
    setLoading(false);
    // Hide the overlay window via Tauri
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().hide();
    } catch {
      // In dev browser mode, just clear state
    }
  };

  return (
    <div className="w-screen h-screen flex items-center justify-center p-4">
      <div className="glass-panel w-full max-w-[400px] rounded-2xl overflow-hidden">
        {/* Red accent top bar */}
        <div className="h-[3px]" style={{ background: 'var(--destructive)' }} />

        <div className="p-5 flex flex-col gap-4">
          {/* Header */}
          <div className="flex items-center gap-2">
            <span
              className="w-2 h-2 rounded-full animate-pulse"
              style={{ background: 'var(--destructive)' }}
            />
            <span className="text-[13px] font-semibold" style={{ color: 'var(--destructive)' }}>
              Focus Session Active
            </span>
          </div>

          {/* App info */}
          <div>
            <div className="text-[15px] font-medium text-primary">
              You switched to {intervention.appName}
            </div>
            {titleExcerpt && (
              <div className="text-[12px] text-muted mt-1 truncate">
                &ldquo;{titleExcerpt}&rdquo;
              </div>
            )}
          </div>

          {/* LLM verdict */}
          {(loading || verdict) && (
            <div className="text-[12px] text-dim flex items-center gap-2">
              {loading && (
                <>
                  <span className="w-3 h-3 border border-dim border-t-transparent rounded-full animate-spin" />
                  Analyzing content...
                </>
              )}
              {verdict && !loading && (
                <span className={verdict.includes('educational') || verdict.includes('research')
                  ? 'text-success' : 'text-destructive'}>
                  {verdict}
                </span>
              )}
            </div>
          )}

          {/* Action buttons */}
          <div className="flex gap-2 mt-1">
            <button
              onClick={handleDismiss}
              className="flex-1 px-3 py-2 rounded-lg text-[12px] font-medium bg-destructive/10 text-destructive hover:bg-destructive/20 transition-colors"
            >
              Back to work
            </button>
            <button
              onClick={handleAllowTemp}
              className="flex-1 px-3 py-2 rounded-lg text-[12px] font-medium bg-surface-raised text-muted hover:text-primary hover:bg-surface-highest transition-colors"
            >
              Allow 5 min
            </button>
            <button
              onClick={handleAllowSession}
              className="flex-1 px-3 py-2 rounded-lg text-[12px] font-medium bg-surface-raised text-muted hover:text-primary hover:bg-surface-highest transition-colors"
            >
              This is work
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Add route in App.tsx**

In `desktop-ui/src/App.tsx`, add import and route:

```tsx
import { DistractionOverlay } from "./components/distraction/DistractionOverlay";

// In the router array:
{ path: "/distraction-overlay", element: <DistractionOverlay /> },
```

**Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: Builds with 0 errors.

**Step 4: Commit**

```bash
git add desktop-ui/src/components/distraction/DistractionOverlay.tsx \
        desktop-ui/src/App.tsx
git commit -m "feat(ui): add distraction overlay component with three-way actions"
```

---

## Task 10: Reset Interceptor on Session End

**Files:**
- Modify: `crates/desktop/src/commands/productivity.rs` (productivity_focus_end)

**Step 1: Clear interceptor state when focus session ends**

In `productivity_focus_end`, after calling `focus_mgr.end_session()`, reset the interceptor:

```rust
#[tauri::command]
pub async fn productivity_focus_end(
    state: State<'_, Arc<AppCore>>,
    notes: Option<String>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    let focus_mgr = state.focus_manager()?;
    let session = focus_mgr.end_session(notes).await.map_err(map_prod_err)?;

    // Clear interceptor session state (whitelist + temp passes)
    if let Ok(interceptor) = state.distraction_interceptor() {
        let mut guard = interceptor.lock().await;
        guard.reset_session();
    }

    Ok(session.map(session_to_response))
}
```

**Step 2: Verify compilation**

Run: `cargo check -p desktop`
Expected: Compiles.

**Step 3: Commit**

```bash
git add crates/desktop/src/commands/productivity.rs
git commit -m "feat(productivity): reset interceptor state when focus session ends"
```

---

## Task 11: Integration Test — Full Flow

**Files:**
- Create: `crates/feature-productivity/src/distraction/tests.rs`
- Modify: `crates/feature-productivity/src/distraction/mod.rs`

This tests the full interceptor flow: alert → heuristic → decision → whitelist → re-evaluate.

**Step 1: Write integration test**

Create `crates/feature-productivity/src/distraction/tests.rs`:

```rust
//! Integration tests for the full distraction interceptor flow.

#[cfg(test)]
mod tests {
    use crate::config::FocusConfig;
    use crate::distraction::interceptor::{DistractionInterceptor, InterceptDecision};
    use crate::repos::learned_rule::{LearnedRule, LearnedRuleRepo};
    use crate::ProductivityFeature;
    use chrono::Utc;

    async fn setup() -> (DistractionInterceptor, LearnedRuleRepo) {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        let repo = LearnedRuleRepo::new(inner);
        let interceptor = DistractionInterceptor::new(FocusConfig::default(), repo.clone());
        (interceptor, repo)
    }

    #[tokio::test]
    async fn full_flow_whitelist_then_reset() {
        let (mut interceptor, _) = setup().await;

        // YouTube triggers overlay
        let d = interceptor.evaluate("Chrome", Some("cat videos - YouTube")).await;
        assert!(matches!(d, InterceptDecision::ShowOverlay { needs_llm: true }));

        // User marks as work-related → whitelisted
        interceptor.whitelist_for_session("cat videos - youtube");
        let d = interceptor.evaluate("Chrome", Some("cat videos - YouTube")).await;
        assert!(matches!(d, InterceptDecision::Allow { .. }));

        // Session ends → whitelist cleared
        interceptor.reset_session();
        let d = interceptor.evaluate("Chrome", Some("cat videos - YouTube")).await;
        assert!(matches!(d, InterceptDecision::ShowOverlay { .. }));
    }

    #[tokio::test]
    async fn learned_rule_auto_allows_after_threshold() {
        let (mut interceptor, repo) = setup().await;
        let now = Utc::now();

        // Insert a learned rule with hit_count >= threshold (default 3)
        let rule = LearnedRule {
            id: None,
            pattern: "react tutorial - youtube".into(),
            pattern_type: "title_keyword".into(),
            classification: "educational".into(),
            confidence: 0.8,
            hit_count: 3,
            last_used_at: now,
            created_at: now,
        };
        repo.insert(&rule).await.unwrap();

        // Should auto-allow
        let d = interceptor
            .evaluate("Chrome", Some("React Tutorial - YouTube"))
            .await;
        assert!(matches!(d, InterceptDecision::Allow { .. }));
    }

    #[tokio::test]
    async fn learned_rule_below_threshold_still_shows_overlay() {
        let (mut interceptor, repo) = setup().await;
        let now = Utc::now();

        let rule = LearnedRule {
            id: None,
            pattern: "some video - youtube".into(),
            pattern_type: "title_keyword".into(),
            classification: "educational".into(),
            confidence: 0.5,
            hit_count: 1, // below threshold
            last_used_at: now,
            created_at: now,
        };
        repo.insert(&rule).await.unwrap();

        let d = interceptor
            .evaluate("Chrome", Some("some video - YouTube"))
            .await;
        assert!(matches!(d, InterceptDecision::ShowOverlay { .. }));
    }
}
```

**Step 2: Add to mod.rs**

In `crates/feature-productivity/src/distraction/mod.rs`:

```rust
pub mod classifier;
pub mod heuristics;
pub mod interceptor;
#[cfg(test)]
mod tests;
```

**Step 3: Run all distraction tests**

Run: `cargo nextest run -p feature-productivity -E 'test(distraction)'`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add crates/feature-productivity/src/distraction/
git commit -m "test(productivity): add integration tests for distraction interceptor flow"
```

---

## Task 12: Final Verification

**Step 1: Run full workspace build**

Run: `cargo build --workspace`
Expected: 0 errors, 0 warnings.

**Step 2: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

**Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

**Step 4: Run format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

**Step 5: Verify frontend**

Run: `cd desktop-ui && bun run build`
Expected: Builds successfully.

**Step 6: Commit any fixes**

If any lint/format fixes needed, commit them.

---

## Summary of All Files

**New files (9):**
- `crates/feature-productivity/migrations/004_distraction_learned_rules.sql`
- `crates/feature-productivity/src/repos/learned_rule.rs`
- `crates/feature-productivity/src/distraction/mod.rs`
- `crates/feature-productivity/src/distraction/heuristics.rs`
- `crates/feature-productivity/src/distraction/interceptor.rs`
- `crates/feature-productivity/src/distraction/classifier.rs`
- `crates/feature-productivity/src/distraction/tests.rs`
- `crates/desktop/src/commands/distraction.rs`
- `desktop-ui/src/components/distraction/DistractionOverlay.tsx`

**Modified files (11):**
- `crates/config/src/schema/productivity.rs`
- `crates/feature-productivity/src/repos/mod.rs`
- `crates/feature-productivity/src/lib.rs`
- `crates/feature-productivity/src/tracker/mod.rs`
- `crates/desktop-shared/src/events.rs`
- `crates/desktop-shared/src/commands.rs`
- `crates/desktop/src/commands/mod.rs`
- `crates/desktop/src/app_core.rs`
- `crates/desktop/src/main.rs`
- `crates/desktop/tauri.conf.json`
- `desktop-ui/src/App.tsx`
