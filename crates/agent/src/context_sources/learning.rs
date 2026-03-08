//! Learning context source — unified user profile, behavioral patterns, and agent adaptations.
//!
//! **DEPRECATED:** Being replaced by `CognitiveContextSource` from the `cognitive` crate,
//! which provides FSRS-scored semantic facts and procedural rules. This source remains
//! active for now because it still provides conversation memory and confidence threshold
//! instructions that the cognitive system does not yet handle.
//!
//! Replaces `MemorySource` (priority 80) + `ConfidenceSource` (priority 50) with a single
//! source that also provides structured user profile data and behavioral patterns.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::{Mutex, RwLock};

use crate::agent_profile::AgentProfile;
use crate::memory::MemoryStore;

/// Default TTL for cached learning context (seconds).
const LEARNING_CACHE_TTL_SECS: i64 = 60;

/// Minimum confidence for user profile entries to appear in context.
pub(crate) const PROFILE_MIN_CONFIDENCE: f64 = 0.5;

/// Minimum sample count for behavioral patterns to appear in context.
pub(crate) const PATTERN_MIN_SAMPLES: i32 = 5;

struct CachedLearning {
    content: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    /// Cache key: agent name (or None). Profile/patterns/adaptations depend on
    /// the active agent, not on the user's message text.
    agent_name: Option<String>,
}

/// Unified learning context source that provides:
/// 1. User profile facts (high confidence entries)
/// 2. Behavioral patterns (reliable patterns)
/// 3. Agent preferences (for current agent)
/// 4. Confidence threshold instructions
/// 5. Conversation memory (via MemoryStore, if enabled)
pub struct LearningContextSource {
    user_profile_repo: storage::UserProfileRepo,
    pattern_repo: storage::BehavioralPatternRepo,
    adaptation_repo: storage::AgentAdaptationRepo,
    confidence_bits: Arc<AtomicU32>,
    conversation_memory: Option<MemoryStore>,
    active_profile: Arc<RwLock<Option<Arc<AgentProfile>>>>,
    cache: Mutex<Option<CachedLearning>>,
}

impl LearningContextSource {
    pub fn new(
        user_profile_repo: storage::UserProfileRepo,
        pattern_repo: storage::BehavioralPatternRepo,
        adaptation_repo: storage::AgentAdaptationRepo,
        confidence_bits: Arc<AtomicU32>,
        conversation_memory: Option<MemoryStore>,
        active_profile: Arc<RwLock<Option<Arc<AgentProfile>>>>,
    ) -> Self {
        Self {
            user_profile_repo,
            pattern_repo,
            adaptation_repo,
            confidence_bits,
            conversation_memory,
            active_profile,
            cache: Mutex::new(None),
        }
    }

    /// Get the shared confidence threshold handle (for LearningService updates).
    pub fn threshold_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.confidence_bits)
    }

    fn threshold(&self) -> f32 {
        f32::from_bits(self.confidence_bits.load(Ordering::Relaxed))
    }
}

#[async_trait]
impl ContextSource for LearningContextSource {
    fn name(&self) -> &str {
        "learning"
    }

    fn priority(&self) -> u8 {
        // Lowered from 60 → 55 so CognitiveContextSource (60) takes precedence.
        // Will be removed once cognitive handles conversation memory + confidence.
        55
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        // Resolve agent name before cache check (short lock)
        let agent_name = self
            .active_profile
            .read()
            .await
            .as_ref()
            .map(|p| p.name.clone());

        // Check TTL cache (keyed on agent name, not message text)
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if Utc::now() < cached.expires_at && cached.agent_name == agent_name {
                    return if cached.content.trim().is_empty() {
                        None
                    } else {
                        Some(cached.content.clone())
                    };
                }
            }
        }

        let mut sections = Vec::new();

        // Run independent DB queries concurrently
        let (profile_result, patterns_result, adaptations_result) = tokio::join!(
            self.user_profile_repo
                .list_above_confidence(PROFILE_MIN_CONFIDENCE),
            self.pattern_repo.list_reliable(PATTERN_MIN_SAMPLES),
            async {
                if let Some(ref name) = agent_name {
                    self.adaptation_repo.list_by_agent(name).await
                } else {
                    Ok(vec![])
                }
            }
        );

        // 1. User profile (high confidence entries)
        if let Ok(entries) = profile_result {
            if !entries.is_empty() {
                let mut profile_lines = vec!["# About the User".to_string()];
                let mut current_category = String::new();

                for entry in &entries {
                    if entry.category != current_category {
                        current_category.clone_from(&entry.category);
                        profile_lines.push(format!("\n## {}", title_case(&current_category)));
                    }
                    let display_value = json_display_value(&entry.value);
                    profile_lines.push(format!("- **{}**: {}", entry.key, display_value));
                }

                sections.push(profile_lines.join("\n"));
            }
        }

        // 2. Behavioral patterns (reliable patterns)
        if let Ok(patterns) = patterns_result {
            if !patterns.is_empty() {
                let mut pattern_lines = vec!["# User Patterns".to_string()];
                for pattern in &patterns {
                    let value = json_display_value(&pattern.pattern_value);
                    pattern_lines.push(format!(
                        "- {}/{}: {} (observed {} times)",
                        pattern.pattern_type, pattern.pattern_key, value, pattern.sample_count
                    ));
                }
                sections.push(pattern_lines.join("\n"));
            }
        }

        // 3. Agent preferences (for current agent)
        if let Some(ref name) = agent_name {
            if let Ok(adaptations) = adaptations_result {
                if !adaptations.is_empty() {
                    let mut adapt_lines =
                        vec![format!("# Preferences for {} Agent", title_case(name))];
                    for adapt in &adaptations {
                        let value = json_display_value(&adapt.preference_value);
                        adapt_lines.push(format!("- **{}**: {}", adapt.preference_key, value));
                    }
                    sections.push(adapt_lines.join("\n"));
                }
            }
        }

        // 4. Confidence threshold
        let threshold = self.threshold();
        sections.push(crate::confidence::prompt::confidence_prompt(threshold));

        // 5. Conversation memory (if enabled)
        if let Some(ref memory) = self.conversation_memory {
            let memory_content = if let Some(ref query) = ctx.message {
                memory.get_relevant_memory(query, 5).await
            } else {
                memory.get_memory_context().await
            };

            if !memory_content.trim().is_empty() {
                sections.push(format!("# Memory\n\n{}", memory_content));
            }
        }

        let content = sections.join("\n\n---\n\n");

        // Cache result
        {
            let mut cache = self.cache.lock().await;
            *cache = Some(CachedLearning {
                content: content.clone(),
                expires_at: Utc::now() + Duration::seconds(LEARNING_CACHE_TTL_SECS),
                agent_name: agent_name.clone(),
            });
        }

        if content.trim().is_empty() {
            None
        } else {
            Some(content)
        }
    }
}

/// Display a JSON-serialized value in a human-readable way.
///
/// Stored values come from `serde_json::to_string()`, so strings are quoted
/// (e.g. `"\"hello\""`). This function deserializes and extracts the inner
/// content, handling strings, numbers, booleans, and compound types.
fn json_display_value(stored: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(stored) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => stored.to_string(),
    }
}

/// Convert snake_case to Title Case for display.
fn title_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_learning_context_includes_user_profile() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        repos
            .user_profile
            .upsert(
                "projects",
                "active_project",
                &serde_json::json!("Klyntbot"),
                "user_explicit",
                1.0,
                None,
            )
            .await
            .unwrap();

        let threshold_bits = Arc::new(AtomicU32::new(0.7_f32.to_bits()));
        let active_profile = Arc::new(RwLock::new(None));

        let source = LearningContextSource::new(
            repos.user_profile.clone(),
            repos.behavioral_patterns.clone(),
            repos.agent_adaptations.clone(),
            threshold_bits,
            None,
            active_profile,
        );
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
        };
        let result = source.provide(&ctx).await;

        assert!(result.is_some());
        let text = result.unwrap();
        assert!(
            text.contains("Klyntbot"),
            "Should contain user profile data"
        );
        assert!(
            text.contains("About the User"),
            "Should contain profile header"
        );
    }

    #[tokio::test]
    async fn test_learning_context_includes_patterns() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        repos
            .behavioral_patterns
            .upsert(
                "day_of_week",
                "monday_tasks",
                &serde_json::json!({"agent": "task"}),
                10,
            )
            .await
            .unwrap();

        let threshold_bits = Arc::new(AtomicU32::new(0.7_f32.to_bits()));
        let active_profile = Arc::new(RwLock::new(None));

        let source = LearningContextSource::new(
            repos.user_profile.clone(),
            repos.behavioral_patterns.clone(),
            repos.agent_adaptations.clone(),
            threshold_bits,
            None,
            active_profile,
        );
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
        };
        let result = source.provide(&ctx).await;

        assert!(result.is_some());
        let text = result.unwrap();
        assert!(
            text.contains("User Patterns"),
            "Should contain patterns section"
        );
        assert!(text.contains("monday_tasks"), "Should contain pattern data");
    }

    #[tokio::test]
    async fn test_learning_context_includes_agent_adaptations() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        repos
            .agent_adaptations
            .upsert(
                "task",
                "response_length",
                &serde_json::json!("concise"),
                "signal",
                0.8,
            )
            .await
            .unwrap();

        let threshold_bits = Arc::new(AtomicU32::new(0.7_f32.to_bits()));
        let task_profile = Arc::new(AgentProfile {
            name: "task".to_string(),
            ..Default::default()
        });
        let active_profile = Arc::new(RwLock::new(Some(task_profile)));

        let source = LearningContextSource::new(
            repos.user_profile.clone(),
            repos.behavioral_patterns.clone(),
            repos.agent_adaptations.clone(),
            threshold_bits,
            None,
            active_profile,
        );
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
        };
        let result = source.provide(&ctx).await;

        assert!(result.is_some());
        let text = result.unwrap();
        assert!(
            text.contains("Preferences for Task Agent"),
            "Should contain agent preferences section"
        );
        assert!(
            text.contains("response_length"),
            "Should contain adaptation data"
        );
    }

    #[tokio::test]
    async fn test_learning_context_caching() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        let threshold_bits = Arc::new(AtomicU32::new(0.7_f32.to_bits()));
        let active_profile = Arc::new(RwLock::new(None));

        let source = LearningContextSource::new(
            repos.user_profile.clone(),
            repos.behavioral_patterns.clone(),
            repos.agent_adaptations.clone(),
            threshold_bits,
            None,
            active_profile,
        );
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
        };

        // First call should populate cache
        let result1 = source.provide(&ctx).await;
        // Second call should hit cache
        let result2 = source.provide(&ctx).await;

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("active_project"), "Active Project");
        assert_eq!(title_case("finance"), "Finance");
        assert_eq!(title_case("day_of_week"), "Day Of Week");
    }
}
