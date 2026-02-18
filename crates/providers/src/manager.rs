//! ProviderManager — failover, retry with backoff, and circuit breaker for LLM providers.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;

use common::{KlyntbotError, ProviderError, Result};

use crate::types::*;

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before the circuit opens
    pub failure_threshold: u32,
    /// Seconds before the circuit resets (half-open → try primary again)
    pub reset_timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout_secs: 60,
        }
    }
}

/// Manages primary/fallback providers with retry, failover, and circuit breaker logic.
pub struct ProviderManager {
    primary: DynProvider,
    fallback: Option<DynProvider>,
    /// Optional dedicated provider for the complexity classifier
    pub classifier_provider: Option<DynProvider>,
    failure_count: Arc<AtomicU32>,
    circuit_open_until: Arc<RwLock<Option<tokio::time::Instant>>>,
    circuit_config: CircuitBreakerConfig,
}

impl ProviderManager {
    pub fn new(
        primary: DynProvider,
        fallback: Option<DynProvider>,
        classifier_provider: Option<DynProvider>,
    ) -> Self {
        Self::with_config(
            primary,
            fallback,
            classifier_provider,
            CircuitBreakerConfig::default(),
        )
    }

    pub fn with_config(
        primary: DynProvider,
        fallback: Option<DynProvider>,
        classifier_provider: Option<DynProvider>,
        circuit_config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            primary,
            fallback,
            classifier_provider,
            failure_count: Arc::new(AtomicU32::new(0)),
            circuit_open_until: Arc::new(RwLock::new(None)),
            circuit_config,
        }
    }

    /// Check if circuit is open (primary should be bypassed)
    async fn is_circuit_open(&self) -> bool {
        let open_until = self.circuit_open_until.read().await;
        match *open_until {
            Some(until) => tokio::time::Instant::now() < until,
            None => false,
        }
    }

    /// Record a failure; opens the circuit if threshold is reached.
    async fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= self.circuit_config.failure_threshold {
            let mut open_until = self.circuit_open_until.write().await;
            *open_until = Some(
                tokio::time::Instant::now()
                    + std::time::Duration::from_secs(self.circuit_config.reset_timeout_secs),
            );
            self.failure_count.store(0, Ordering::SeqCst);
        }
    }

    /// Reset consecutive failure counter on success.
    fn reset_failures(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
    }

    /// Try primary with exponential backoff on rate-limit errors.
    /// 3 attempts max, delays: 500ms → 1s → 2s. Non-rate-limit errors fail fast.
    async fn try_primary_with_retry(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmResponse> {
        let delays = [
            std::time::Duration::from_millis(500),
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        ];

        let mut last_err = None;
        for (attempt, delay) in delays.iter().enumerate() {
            match self.primary.chat(messages, tools, params).await {
                Ok(response) => {
                    self.reset_failures();
                    return Ok(response);
                }
                Err(KlyntbotError::Provider(ProviderError::RateLimited)) => {
                    last_err = Some(KlyntbotError::Provider(ProviderError::RateLimited));
                    if attempt < delays.len() - 1 {
                        tokio::time::sleep(*delay).await;
                    }
                }
                Err(e) => {
                    // Non-rate-limit error: record failure and bail
                    self.record_failure().await;
                    return Err(e);
                }
            }
        }
        // Exhausted all retries — record failure
        self.record_failure().await;
        Err(last_err.unwrap())
    }

    /// Route to fallback if available, otherwise return the original error.
    async fn try_fallback(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        primary_err: KlyntbotError,
    ) -> Result<LlmResponse> {
        match &self.fallback {
            Some(fb) => fb.chat(messages, tools, params).await,
            None => Err(primary_err),
        }
    }
}

#[async_trait]
impl LlmProvider for ProviderManager {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmResponse> {
        // Circuit open → skip primary entirely
        if self.is_circuit_open().await {
            if let Some(fb) = &self.fallback {
                return fb.chat(messages, tools, params).await;
            }
        }

        match self.try_primary_with_retry(messages, tools, params).await {
            Ok(r) => Ok(r),
            Err(e) => self.try_fallback(messages, tools, params, e).await,
        }
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmStream> {
        if self.is_circuit_open().await {
            if let Some(fb) = &self.fallback {
                return fb.chat_stream(messages, tools, params).await;
            }
        }
        match self.primary.chat_stream(messages, tools, params).await {
            Ok(s) => Ok(s),
            Err(e) => {
                if let Some(fb) = &self.fallback {
                    fb.chat_stream(messages, tools, params).await
                } else {
                    Err(e)
                }
            }
        }
    }

    fn supports_streaming(&self) -> bool {
        self.primary.supports_streaming()
    }

    fn default_model(&self) -> &str {
        self.primary.default_model()
    }

    fn name(&self) -> &str {
        "provider-manager"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.primary.capabilities()
    }

    fn context_window(&self) -> usize {
        self.primary.context_window()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    /// A test provider that counts calls and optionally returns a configured error.
    struct CountingProvider {
        call_count: Arc<AtomicUsize>,
        fail_with: Option<fn() -> KlyntbotError>,
        label: &'static str,
    }

    impl CountingProvider {
        fn ok(label: &'static str, counter: Arc<AtomicUsize>) -> Self {
            Self {
                call_count: counter,
                fail_with: None,
                label,
            }
        }

        fn failing(
            label: &'static str,
            counter: Arc<AtomicUsize>,
            err_fn: fn() -> KlyntbotError,
        ) -> Self {
            Self {
                call_count: counter,
                fail_with: Some(err_fn),
                label,
            }
        }
    }

    fn dummy_response() -> LlmResponse {
        LlmResponse {
            content: Some("ok".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    fn rate_limited_error() -> KlyntbotError {
        KlyntbotError::Provider(ProviderError::RateLimited)
    }

    fn auth_error() -> KlyntbotError {
        KlyntbotError::Provider(ProviderError::AuthFailed)
    }

    #[async_trait]
    impl LlmProvider for CountingProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
            match self.fail_with {
                Some(err_fn) => Err(err_fn()),
                None => Ok(dummy_response()),
            }
        }

        fn default_model(&self) -> &str {
            "test-model"
        }

        fn name(&self) -> &str {
            self.label
        }
    }

    // ── Tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_primary_provider_used_first() {
        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::ok("primary", primary_count.clone())),
            Some(Arc::new(CountingProvider::ok(
                "fallback",
                fallback_count.clone(),
            ))),
            None,
        );

        let result = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert!(result.is_ok());
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(fallback_count.load(AtomicOrdering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_fallback_on_rate_limit() {
        // Use tokio::time::pause() so retry sleeps are instant
        tokio::time::pause();

        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                rate_limited_error,
            )),
            Some(Arc::new(CountingProvider::ok(
                "fallback",
                fallback_count.clone(),
            ))),
            None,
        );

        let result = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert!(result.is_ok());
        // Primary should have been tried 3 times (retry with backoff)
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 3);
        // Fallback should have been called once after retries exhausted
        assert_eq!(fallback_count.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_fallback_on_non_retryable_error() {
        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                auth_error,
            )),
            Some(Arc::new(CountingProvider::ok(
                "fallback",
                fallback_count.clone(),
            ))),
            None,
        );

        let result = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert!(result.is_ok());
        // Non-retryable: primary tried once, then fallback
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(fallback_count.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_no_fallback_returns_error() {
        let primary_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                auth_error,
            )),
            None, // no fallback
            None,
        );

        let result = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert!(result.is_err());
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        tokio::time::pause();

        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::with_config(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                auth_error, // non-retryable → 1 attempt per call, records failure
            )),
            Some(Arc::new(CountingProvider::ok(
                "fallback",
                fallback_count.clone(),
            ))),
            None,
            CircuitBreakerConfig {
                failure_threshold: 3,
                reset_timeout_secs: 60,
            },
        );

        // Make 5 calls. After 3 failures, circuit opens → subsequent calls skip primary.
        for _ in 0..5 {
            let _ = manager
                .chat(&[], None, &ChatParams::new("test-model"))
                .await;
        }

        let primary_calls = primary_count.load(AtomicOrdering::SeqCst);
        let fallback_calls = fallback_count.load(AtomicOrdering::SeqCst);

        // Primary should have been called exactly 3 times (threshold), then circuit opens
        assert_eq!(primary_calls, 3);
        // Fallback handles all 5 calls (3 as failover + 2 as circuit-open bypass)
        assert_eq!(fallback_calls, 5);
    }

    #[tokio::test]
    async fn test_circuit_resets_after_timeout() {
        tokio::time::pause();

        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::with_config(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                auth_error,
            )),
            Some(Arc::new(CountingProvider::ok(
                "fallback",
                fallback_count.clone(),
            ))),
            None,
            CircuitBreakerConfig {
                failure_threshold: 2,
                reset_timeout_secs: 10,
            },
        );

        // Trip the circuit (2 failures)
        for _ in 0..2 {
            let _ = manager
                .chat(&[], None, &ChatParams::new("test-model"))
                .await;
        }
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 2);

        // Call while circuit is open → goes to fallback without touching primary
        let _ = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 2); // unchanged

        // Advance past reset timeout
        tokio::time::advance(std::time::Duration::from_secs(11)).await;

        // Circuit should be closed now → primary gets tried again
        let _ = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_with_backoff_succeeds_on_second_attempt() {
        tokio::time::pause();

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        // Provider that fails on first call, succeeds on subsequent
        struct FirstFailProvider {
            call_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl LlmProvider for FirstFailProvider {
            async fn chat(
                &self,
                _messages: &[Message],
                _tools: Option<&[Value]>,
                _params: &ChatParams,
            ) -> Result<LlmResponse> {
                let n = self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
                if n == 0 {
                    Err(KlyntbotError::Provider(ProviderError::RateLimited))
                } else {
                    Ok(dummy_response())
                }
            }
            fn default_model(&self) -> &str {
                "test"
            }
            fn name(&self) -> &str {
                "first-fail"
            }
        }

        let manager = ProviderManager::new(
            Arc::new(FirstFailProvider {
                call_count: call_count_clone,
            }),
            None,
            None,
        );

        let result = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert!(result.is_ok());
        assert_eq!(call_count.load(AtomicOrdering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_success_resets_failure_counter() {
        let primary_count = Arc::new(AtomicUsize::new(0));

        // Provider that alternates: fail, fail, succeed
        struct AlternatingProvider {
            call_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl LlmProvider for AlternatingProvider {
            async fn chat(
                &self,
                _messages: &[Message],
                _tools: Option<&[Value]>,
                _params: &ChatParams,
            ) -> Result<LlmResponse> {
                let n = self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
                if n % 3 == 2 {
                    Ok(dummy_response())
                } else {
                    Err(KlyntbotError::Provider(ProviderError::AuthFailed))
                }
            }
            fn default_model(&self) -> &str {
                "test"
            }
            fn name(&self) -> &str {
                "alternating"
            }
        }

        let manager = ProviderManager::with_config(
            Arc::new(AlternatingProvider {
                call_count: primary_count.clone(),
            }),
            None,
            None,
            CircuitBreakerConfig {
                failure_threshold: 3,
                reset_timeout_secs: 60,
            },
        );

        // Call 1: fail (failures=1)
        let _ = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        // Call 2: fail (failures=2)
        let _ = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        // Call 3: succeed → resets counter
        let r = manager
            .chat(&[], None, &ChatParams::new("test-model"))
            .await;
        assert!(r.is_ok());

        // Circuit should NOT have opened because success reset the counter
        assert!(!manager.is_circuit_open().await);
    }

    #[tokio::test]
    async fn test_provider_manager_delegates_name_and_model() {
        struct NamedProvider;

        #[async_trait]
        impl LlmProvider for NamedProvider {
            async fn chat(
                &self,
                _: &[Message],
                _: Option<&[Value]>,
                _: &ChatParams,
            ) -> Result<LlmResponse> {
                Ok(dummy_response())
            }
            fn default_model(&self) -> &str {
                "my-model-v1"
            }
            fn name(&self) -> &str {
                "named-provider"
            }
        }

        let manager = ProviderManager::new(Arc::new(NamedProvider), None, None);

        assert_eq!(manager.name(), "provider-manager");
        assert_eq!(manager.default_model(), "my-model-v1");
    }
}
