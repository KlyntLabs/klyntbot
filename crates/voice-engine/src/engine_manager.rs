//! Voice engine manager with primary/fallback routing and circuit breaker.
//!
//! Wraps a primary TTS engine with an optional fallback. When the primary
//! fails repeatedly, the circuit opens and requests route to the fallback.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::tts::TtsEngine;
use crate::types::*;

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures before the circuit opens.
    pub failure_threshold: u32,
    /// Seconds before trying the primary again (half-open).
    pub reset_timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            reset_timeout_secs: 30,
        }
    }
}

/// Manages a primary TTS engine with an optional fallback.
pub struct TtsEngineManager {
    primary: Arc<dyn TtsEngine>,
    fallback: Option<Arc<dyn TtsEngine>>,
    failure_count: AtomicU32,
    circuit_open_until: RwLock<Option<tokio::time::Instant>>,
    config: CircuitBreakerConfig,
}

impl TtsEngineManager {
    pub fn new(primary: Arc<dyn TtsEngine>, fallback: Option<Arc<dyn TtsEngine>>) -> Self {
        Self {
            primary,
            fallback,
            failure_count: AtomicU32::new(0),
            circuit_open_until: RwLock::new(None),
            config: CircuitBreakerConfig::default(),
        }
    }

    pub fn with_config(
        primary: Arc<dyn TtsEngine>,
        fallback: Option<Arc<dyn TtsEngine>>,
        config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            primary,
            fallback,
            failure_count: AtomicU32::new(0),
            circuit_open_until: RwLock::new(None),
            config,
        }
    }

    async fn is_circuit_open(&self) -> bool {
        // Check with read lock first (fast path)
        {
            let guard = self.circuit_open_until.read().await;
            match *guard {
                Some(deadline) if tokio::time::Instant::now() < deadline => return true,
                None => return false,
                _ => {} // expired — fall through to reset
            }
        }
        // Expired: acquire write lock atomically to reset (avoids TOCTOU)
        let mut guard = self.circuit_open_until.write().await;
        if guard.is_some() {
            *guard = None;
            self.failure_count.store(0, Ordering::Relaxed);
        }
        false
    }

    fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.config.failure_threshold {
            let deadline = tokio::time::Instant::now()
                + std::time::Duration::from_secs(self.config.reset_timeout_secs);
            if let Ok(mut guard) = self.circuit_open_until.try_write() {
                *guard = Some(deadline);
                warn!(
                    "TTS circuit breaker opened after {count} failures, retry in {}s",
                    self.config.reset_timeout_secs
                );
            }
        }
    }

    fn reset_failures(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl TtsEngine for TtsEngineManager {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        if !self.is_circuit_open().await {
            match self.primary.synthesize(text, params).await {
                Ok(clip) => {
                    self.reset_failures();
                    return Ok(clip);
                }
                Err(e) => {
                    self.record_failure();
                    warn!("Primary TTS failed: {e}");
                    if let Some(ref fallback) = self.fallback {
                        info!("Falling back to {}", fallback.display_name());
                        return fallback.synthesize(text, params).await;
                    }
                    return Err(e);
                }
            }
        }

        // Circuit is open — use fallback directly
        match self.fallback {
            Some(ref fallback) => fallback.synthesize(text, params).await,
            None => Err(common::KlyntbotError::Timeout(
                "TTS circuit breaker open and no fallback configured".to_string(),
            )),
        }
    }

    fn supports_language(&self, lang: &Language) -> bool {
        self.primary.supports_language(lang)
            || self
                .fallback
                .as_ref()
                .is_some_and(|f| f.supports_language(lang))
    }

    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo> {
        let mut voices = self.primary.available_voices(lang);
        if let Some(ref fallback) = self.fallback {
            voices.extend(fallback.available_voices(lang));
        }
        voices
    }

    fn display_name(&self) -> &str {
        self.primary.display_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockTtsEngine;

    struct FailingTts;

    #[async_trait]
    impl TtsEngine for FailingTts {
        async fn synthesize(&self, _text: &str, _params: &TtsParams) -> common::Result<AudioClip> {
            Err(common::KlyntbotError::Timeout("always fails".to_string()))
        }
        fn supports_language(&self, _: &Language) -> bool {
            false
        }
        fn available_voices(&self, _: &Language) -> Vec<VoiceInfo> {
            vec![]
        }
        fn display_name(&self) -> &str {
            "Failing"
        }
    }

    #[tokio::test]
    async fn primary_succeeds() {
        let primary = Arc::new(MockTtsEngine) as Arc<dyn TtsEngine>;
        let manager = TtsEngineManager::new(primary, None);

        let clip = manager
            .synthesize("hello", &TtsParams::default())
            .await
            .unwrap();
        assert!(!clip.samples.is_empty());
    }

    #[tokio::test]
    async fn display_name_from_primary() {
        let primary = Arc::new(MockTtsEngine) as Arc<dyn TtsEngine>;
        let manager = TtsEngineManager::new(primary, None);
        assert_eq!(manager.display_name(), "Mock");
    }

    #[tokio::test]
    async fn fallback_used_when_primary_fails() {
        let primary = Arc::new(FailingTts) as Arc<dyn TtsEngine>;
        let fallback = Arc::new(MockTtsEngine) as Arc<dyn TtsEngine>;
        let manager = TtsEngineManager::new(primary, Some(fallback));

        let clip = manager
            .synthesize("hello", &TtsParams::default())
            .await
            .unwrap();
        assert!(!clip.samples.is_empty());
    }

    #[tokio::test]
    async fn circuit_opens_after_threshold() {
        let primary = Arc::new(FailingTts) as Arc<dyn TtsEngine>;
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout_secs: 60,
        };
        let manager = TtsEngineManager::with_config(primary, None, config);

        let _ = manager.synthesize("a", &TtsParams::default()).await;
        let _ = manager.synthesize("b", &TtsParams::default()).await;

        assert!(manager.is_circuit_open().await);
    }
}
