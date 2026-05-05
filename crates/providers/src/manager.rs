//! ProviderManager — failover, retry with backoff, and circuit breaker for LLM providers.

use std::future::Future;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use jiff::Timestamp;
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

/// Called when the circuit opens. Receives the UTC wall-clock deadline.
/// Used by app-core to persist state across restarts.
pub type OnCircuitOpen = Arc<dyn Fn(Timestamp) + Send + Sync>;

/// Degradation level emitted by [`OnProviderDegraded`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationLevel {
    /// Primary circuit opened; requests are being routed to the fallback provider.
    Fallback,
    /// All providers have failed; no LLM calls can succeed.
    Offline,
}

/// Called when provider degradation is detected.
/// Used by app-core to forward a `provider:degraded` Tauri event to the frontend.
pub type OnProviderDegraded = Arc<dyn Fn(DegradationLevel) + Send + Sync>;

/// Manages primary/fallback providers with retry, failover, and circuit breaker logic.
pub struct ProviderManager {
    primary: DynProvider,
    fallback: Option<DynProvider>,
    /// Optional dedicated provider for the complexity classifier
    pub classifier_provider: Option<DynProvider>,
    failure_count: Arc<AtomicU32>,
    circuit_open_until: Arc<RwLock<Option<tokio::time::Instant>>>,
    /// Wall-clock counterpart to `circuit_open_until` — serializable for persistence.
    circuit_open_until_utc: Arc<RwLock<Option<Timestamp>>>,
    circuit_config: CircuitBreakerConfig,
    /// Optional callback invoked when the circuit opens, for persistence.
    /// Stored behind RwLock so it can be set after Arc construction.
    on_circuit_open: RwLock<Option<OnCircuitOpen>>,
    /// Optional callback invoked on provider degradation (fallback / offline).
    /// Stored behind RwLock so it can be set after Arc construction.
    on_provider_degraded: RwLock<Option<OnProviderDegraded>>,
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
            circuit_open_until_utc: Arc::new(RwLock::new(None)),
            circuit_config,
            on_circuit_open: RwLock::new(None),
            on_provider_degraded: RwLock::new(None),
        }
    }

    /// Attach a callback invoked when the circuit opens (used by app-core for persistence).
    /// Can be called after `Arc` construction.
    pub async fn set_circuit_open_callback(&self, callback: OnCircuitOpen) {
        *self.on_circuit_open.write().await = Some(callback);
    }

    /// Attach a callback invoked when provider degradation is detected.
    /// Used by app-core to forward `provider:degraded` events to the frontend.
    /// Can be called after `Arc` construction.
    pub async fn set_provider_degraded_callback(&self, callback: OnProviderDegraded) {
        *self.on_provider_degraded.write().await = Some(callback);
    }

    /// Restore circuit breaker state from a persisted UTC deadline.
    /// Call this on startup after loading from storage. No-ops if deadline has already passed.
    pub async fn restore_circuit_state(&self, open_until_utc: Timestamp) {
        let now = Timestamp::now();
        let remaining_ms = open_until_utc.as_millisecond() - now.as_millisecond();
        if remaining_ms <= 0 {
            return; // already expired — treat as closed
        }
        let duration = std::time::Duration::from_millis(remaining_ms as u64);
        *self.circuit_open_until.write().await = Some(tokio::time::Instant::now() + duration);
        *self.circuit_open_until_utc.write().await = Some(open_until_utc);
        tracing::info!(
            open_until = %open_until_utc,
            "circuit breaker restored from persisted state"
        );
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
            let reset_dur = std::time::Duration::from_secs(self.circuit_config.reset_timeout_secs);
            let jiff_dur = jiff::SignedDuration::try_from(reset_dur)
                .unwrap_or(jiff::SignedDuration::from_secs(60));
            let open_until_utc = Timestamp::now() + jiff_dur;

            *self.circuit_open_until.write().await = Some(tokio::time::Instant::now() + reset_dur);
            *self.circuit_open_until_utc.write().await = Some(open_until_utc);
            self.failure_count.store(0, Ordering::SeqCst);

            if let Some(ref cb) = *self.on_circuit_open.read().await {
                cb(open_until_utc);
            }

            // Notify listeners that the primary provider has degraded to fallback.
            if let Some(ref cb) = *self.on_provider_degraded.read().await {
                cb(DegradationLevel::Fallback);
            }
        }
    }

    /// Reset consecutive failure counter on success.
    fn reset_failures(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
    }

    /// Exponential backoff on rate-limit errors: 3 attempts, delays 500ms → 1s → 2s.
    /// Non-rate-limit errors fail fast. Wraps any async call via a closure.
    async fn retry_with_backoff<F, Fut, T>(&self, call: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let delays = [
            std::time::Duration::from_millis(500),
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        ];
        self.retry_with_backoff_inner(&delays, true, call).await
    }

    /// Retry with custom delay schedule, optionally updating the primary circuit breaker.
    ///
    /// When `update_circuit_breaker` is false, failures and successes do not
    /// affect the primary provider's circuit breaker state — used for fallback
    /// retries that should not influence primary health tracking.
    async fn retry_with_backoff_inner<F, Fut, T>(
        &self,
        delays: &[std::time::Duration],
        update_circuit_breaker: bool,
        call: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut last_err = None;
        for (attempt, delay) in delays.iter().enumerate() {
            match call().await {
                Ok(val) => {
                    if update_circuit_breaker {
                        self.reset_failures();
                    }
                    return Ok(val);
                }
                Err(e @ KlyntbotError::Provider(ProviderError::RateLimited { .. })) => {
                    last_err = Some(e);
                    if attempt < delays.len() - 1 {
                        tokio::time::sleep(*delay).await;
                    }
                }
                Err(e) => {
                    if update_circuit_breaker {
                        self.record_failure().await;
                    }
                    return Err(e);
                }
            }
        }
        if update_circuit_breaker {
            self.record_failure().await;
        }
        Err(last_err.unwrap())
    }

    async fn try_primary_with_retry(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        self.retry_with_backoff(|| {
            self.primary
                .chat(messages, tools, params, cache_breakpoints)
        })
        .await
    }

    async fn try_primary_stream_with_retry(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmStream> {
        self.retry_with_backoff(|| {
            self.primary
                .chat_stream(messages, tools, params, cache_breakpoints)
        })
        .await
    }

    /// Check health of primary and fallback providers.
    ///
    /// Returns a tuple of `(primary_health, fallback_health)`.
    /// If no fallback is configured, the second element is `None`.
    pub async fn check_health(&self) -> (ProviderHealth, Option<ProviderHealth>) {
        let primary_health = self
            .primary
            .health_check()
            .await
            .unwrap_or(ProviderHealth::Unknown);

        let fallback_health = if let Some(fb) = &self.fallback {
            Some(fb.health_check().await.unwrap_or(ProviderHealth::Unknown))
        } else {
            None
        };

        (primary_health, fallback_health)
    }

    /// Route to fallback if available, otherwise return the original error.
    /// Retries the fallback up to 2 times on rate-limit errors without
    /// affecting the primary provider's circuit breaker state.
    /// Emits [`DegradationLevel::Offline`] when no provider can serve the request.
    async fn try_fallback(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
        primary_err: KlyntbotError,
    ) -> Result<LlmResponse> {
        let delays = [
            std::time::Duration::from_millis(500),
            std::time::Duration::from_secs(1),
        ];
        match &self.fallback {
            Some(fb) => {
                let result = self
                    .retry_with_backoff_inner(&delays, false, || {
                        fb.chat(messages, tools, params, cache_breakpoints)
                    })
                    .await;
                if result.is_err() {
                    if let Some(ref cb) = *self.on_provider_degraded.read().await {
                        cb(DegradationLevel::Offline);
                    }
                }
                result
            }
            None => {
                if let Some(ref cb) = *self.on_provider_degraded.read().await {
                    cb(DegradationLevel::Offline);
                }
                Err(primary_err)
            }
        }
    }
}

impl ProviderManager {
    /// Call `chat` with an explicit provider role.
    ///
    /// Currently routes to the default provider; future work can resolve
    /// role-specific providers via a registry.
    pub async fn chat_with_role(
        &self,
        role: crate::ProviderRole,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        let mut params = params.clone();
        params.role = Some(role);
        self.chat(messages, tools, &params, cache_breakpoints).await
    }
}

#[async_trait]
impl LlmProvider for ProviderManager {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        // Circuit open → skip primary entirely
        if self.is_circuit_open().await {
            if let Some(fb) = &self.fallback {
                return fb.chat(messages, tools, params, cache_breakpoints).await;
            }
        }

        match self
            .try_primary_with_retry(messages, tools, params, cache_breakpoints)
            .await
        {
            Ok(r) => Ok(r),
            Err(e) => {
                self.try_fallback(messages, tools, params, cache_breakpoints, e)
                    .await
            }
        }
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmStream> {
        // Circuit open → skip primary entirely
        if self.is_circuit_open().await {
            if let Some(fb) = &self.fallback {
                return fb
                    .chat_stream(messages, tools, params, cache_breakpoints)
                    .await;
            }
        }

        match self
            .try_primary_stream_with_retry(messages, tools, params, cache_breakpoints)
            .await
        {
            Ok(s) => Ok(s),
            Err(e) => match &self.fallback {
                Some(fb) => {
                    let result = fb
                        .chat_stream(messages, tools, params, cache_breakpoints)
                        .await;
                    if result.is_err() {
                        if let Some(ref cb) = *self.on_provider_degraded.read().await {
                            cb(DegradationLevel::Offline);
                        }
                    }
                    result
                }
                None => {
                    if let Some(ref cb) = *self.on_provider_degraded.read().await {
                        cb(DegradationLevel::Offline);
                    }
                    Err(e)
                }
            },
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

    async fn health_check(&self) -> Result<ProviderHealth> {
        let (primary, _fallback) = self.check_health().await;
        Ok(primary)
    }

    fn classifier_provider(&self) -> Option<DynProvider> {
        self.classifier_provider.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    /// A test provider that counts calls and optionally returns a configured error.
    /// Tracks chat and chat_stream calls via separate counters.
    struct CountingProvider {
        call_count: Arc<AtomicUsize>,
        stream_call_count: Arc<AtomicUsize>,
        fail_with: Option<fn() -> KlyntbotError>,
        label: &'static str,
    }

    impl CountingProvider {
        fn ok(label: &'static str, counter: Arc<AtomicUsize>) -> Self {
            Self {
                call_count: counter,
                stream_call_count: Arc::new(AtomicUsize::new(0)),
                fail_with: None,
                label,
            }
        }

        fn ok_streaming(
            label: &'static str,
            counter: Arc<AtomicUsize>,
            stream_counter: Arc<AtomicUsize>,
        ) -> Self {
            Self {
                call_count: counter,
                stream_call_count: stream_counter,
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
                stream_call_count: Arc::new(AtomicUsize::new(0)),
                fail_with: Some(err_fn),
                label,
            }
        }

        fn failing_streaming(
            label: &'static str,
            stream_counter: Arc<AtomicUsize>,
            err_fn: fn() -> KlyntbotError,
        ) -> Self {
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
                stream_call_count: stream_counter,
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

    fn dummy_stream() -> LlmStream {
        let chunk = LlmStreamChunk {
            content: Some("ok".to_string()),
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some("stop".to_string()),
            reasoning_content: None,
            usage: None,
        };
        Box::pin(futures_util::stream::once(async move { Ok(chunk) }))
    }

    fn rate_limited_error() -> KlyntbotError {
        KlyntbotError::Provider(ProviderError::RateLimited {
            provider: "test".into(),
            retry_after: None,
        })
    }

    fn auth_error() -> KlyntbotError {
        KlyntbotError::Provider(ProviderError::AuthFailed {
            provider: "test".into(),
            config_key: "providers.test.apiKey".into(),
        })
    }

    #[async_trait]
    impl LlmProvider for CountingProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
            _cache_breakpoints: &[CacheBreakpoint],
        ) -> Result<LlmResponse> {
            self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
            match self.fail_with {
                Some(err_fn) => Err(err_fn()),
                None => Ok(dummy_response()),
            }
        }

        async fn chat_stream(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
            _cache_breakpoints: &[CacheBreakpoint],
        ) -> Result<LlmStream> {
            self.stream_call_count.fetch_add(1, AtomicOrdering::SeqCst);
            match self.fail_with {
                Some(err_fn) => Err(err_fn()),
                None => Ok(dummy_stream()),
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
            .chat(&[], None, &ChatParams::new("test-model"), &[])
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
            .chat(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_ok());
        // Primary should have been tried 3 times (retry with backoff)
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 3);
        // Fallback should have been called once after retries exhausted
        assert_eq!(fallback_count.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_fallback_retries_on_rate_limit() {
        tokio::time::pause();

        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                auth_error, // non-retryable → fails fast to reach fallback
            )),
            Some(Arc::new(CountingProvider::failing(
                "fallback",
                fallback_count.clone(),
                rate_limited_error, // fallback is rate-limited
            ))),
            None,
        );

        let result = manager
            .chat(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        // Should fail — fallback exhausted its retries
        assert!(result.is_err());
        // Primary tried once (non-retryable)
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 1);
        // Fallback retried 2 times (delays [500ms, 1s])
        assert_eq!(fallback_count.load(AtomicOrdering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_fallback_retry_succeeds_on_second_attempt() {
        tokio::time::pause();

        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_call_count = Arc::new(AtomicUsize::new(0));
        let fallback_call_count_clone = fallback_call_count.clone();

        /// Fallback provider that fails with rate-limit on first call, then succeeds.
        struct FirstFailFallback {
            call_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl LlmProvider for FirstFailFallback {
            async fn chat(
                &self,
                _messages: &[Message],
                _tools: Option<&[Value]>,
                _params: &ChatParams,
                _cache_breakpoints: &[CacheBreakpoint],
            ) -> Result<LlmResponse> {
                let n = self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
                if n == 0 {
                    Err(KlyntbotError::Provider(ProviderError::RateLimited {
                        provider: "fallback".into(),
                        retry_after: None,
                    }))
                } else {
                    Ok(dummy_response())
                }
            }
            fn default_model(&self) -> &str {
                "test"
            }
            fn name(&self) -> &str {
                "first-fail-fallback"
            }
        }

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                auth_error,
            )),
            Some(Arc::new(FirstFailFallback {
                call_count: fallback_call_count_clone,
            })),
            None,
        );

        let result = manager
            .chat(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_ok());
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 1);
        // Fallback: first call rate-limited, second succeeds
        assert_eq!(fallback_call_count.load(AtomicOrdering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_fallback_retry_does_not_affect_circuit_breaker() {
        tokio::time::pause();

        let primary_count = Arc::new(AtomicUsize::new(0));
        let fallback_count = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::with_config(
            Arc::new(CountingProvider::failing(
                "primary",
                primary_count.clone(),
                auth_error,
            )),
            Some(Arc::new(CountingProvider::failing(
                "fallback",
                fallback_count.clone(),
                rate_limited_error,
            ))),
            None,
            CircuitBreakerConfig {
                failure_threshold: 3,
                reset_timeout_secs: 60,
            },
        );

        // Each call: primary fails (records 1 failure), fallback rate-limited (no circuit effect)
        for _ in 0..2 {
            let _ = manager
                .chat(&[], None, &ChatParams::new("test-model"), &[])
                .await;
        }

        // Primary failed twice → circuit should NOT be open (threshold is 3)
        // If fallback retries were incorrectly touching the circuit breaker,
        // the extra failures would have tripped it.
        assert!(!manager.is_circuit_open().await);
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 2);
        // Each fallback call retried 2 times
        assert_eq!(fallback_count.load(AtomicOrdering::SeqCst), 4);
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
            .chat(&[], None, &ChatParams::new("test-model"), &[])
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
            .chat(&[], None, &ChatParams::new("test-model"), &[])
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
                .chat(&[], None, &ChatParams::new("test-model"), &[])
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
                .chat(&[], None, &ChatParams::new("test-model"), &[])
                .await;
        }
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 2);

        // Call while circuit is open → goes to fallback without touching primary
        let _ = manager
            .chat(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert_eq!(primary_count.load(AtomicOrdering::SeqCst), 2); // unchanged

        // Advance past reset timeout
        tokio::time::advance(std::time::Duration::from_secs(11)).await;

        // Circuit should be closed now → primary gets tried again
        let _ = manager
            .chat(&[], None, &ChatParams::new("test-model"), &[])
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
                _cache_breakpoints: &[CacheBreakpoint],
            ) -> Result<LlmResponse> {
                let n = self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
                if n == 0 {
                    Err(KlyntbotError::Provider(ProviderError::RateLimited {
                        provider: "first-fail".into(),
                        retry_after: None,
                    }))
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
            .chat(&[], None, &ChatParams::new("test-model"), &[])
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
                _cache_breakpoints: &[CacheBreakpoint],
            ) -> Result<LlmResponse> {
                let n = self.call_count.fetch_add(1, AtomicOrdering::SeqCst);
                if n % 3 == 2 {
                    Ok(dummy_response())
                } else {
                    Err(KlyntbotError::Provider(ProviderError::AuthFailed {
                        provider: "alternating".into(),
                        config_key: "providers.alternating.apiKey".into(),
                    }))
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
            .chat(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        // Call 2: fail (failures=2)
        let _ = manager
            .chat(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        // Call 3: succeed → resets counter
        let r = manager
            .chat(&[], None, &ChatParams::new("test-model"), &[])
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
                _: &[CacheBreakpoint],
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

    // ── Streaming retry tests ─────────────────────────────

    #[tokio::test]
    async fn test_stream_primary_used_first() {
        let primary_stream = Arc::new(AtomicUsize::new(0));
        let fallback_stream = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::ok_streaming(
                "primary",
                Arc::new(AtomicUsize::new(0)),
                primary_stream.clone(),
            )),
            Some(Arc::new(CountingProvider::ok_streaming(
                "fallback",
                Arc::new(AtomicUsize::new(0)),
                fallback_stream.clone(),
            ))),
            None,
        );

        let result = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_ok());
        assert_eq!(primary_stream.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(fallback_stream.load(AtomicOrdering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_stream_fallback_on_rate_limit() {
        tokio::time::pause();

        let primary_stream = Arc::new(AtomicUsize::new(0));
        let fallback_stream = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing_streaming(
                "primary",
                primary_stream.clone(),
                rate_limited_error,
            )),
            Some(Arc::new(CountingProvider::ok_streaming(
                "fallback",
                Arc::new(AtomicUsize::new(0)),
                fallback_stream.clone(),
            ))),
            None,
        );

        let result = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_ok());
        // Primary retried 3 times with backoff
        assert_eq!(primary_stream.load(AtomicOrdering::SeqCst), 3);
        // Fallback called once after retries exhausted
        assert_eq!(fallback_stream.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_stream_no_retry_on_non_retryable_error() {
        let primary_stream = Arc::new(AtomicUsize::new(0));
        let fallback_stream = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing_streaming(
                "primary",
                primary_stream.clone(),
                auth_error,
            )),
            Some(Arc::new(CountingProvider::ok_streaming(
                "fallback",
                Arc::new(AtomicUsize::new(0)),
                fallback_stream.clone(),
            ))),
            None,
        );

        let result = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_ok());
        // Non-retryable: primary tried once, then fallback
        assert_eq!(primary_stream.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(fallback_stream.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_stream_no_fallback_returns_error() {
        let primary_stream = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::new(
            Arc::new(CountingProvider::failing_streaming(
                "primary",
                primary_stream.clone(),
                auth_error,
            )),
            None,
            None,
        );

        let result = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_err());
        assert_eq!(primary_stream.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_stream_retry_succeeds_on_second_attempt() {
        tokio::time::pause();

        let stream_count = Arc::new(AtomicUsize::new(0));
        let stream_count_clone = stream_count.clone();

        struct FirstFailStreamProvider {
            stream_count: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl LlmProvider for FirstFailStreamProvider {
            async fn chat(
                &self,
                _: &[Message],
                _: Option<&[Value]>,
                _: &ChatParams,
                _: &[CacheBreakpoint],
            ) -> Result<LlmResponse> {
                Ok(dummy_response())
            }
            async fn chat_stream(
                &self,
                _: &[Message],
                _: Option<&[Value]>,
                _: &ChatParams,
                _: &[CacheBreakpoint],
            ) -> Result<LlmStream> {
                let n = self.stream_count.fetch_add(1, AtomicOrdering::SeqCst);
                if n == 0 {
                    Err(KlyntbotError::Provider(ProviderError::RateLimited {
                        provider: "first-fail-stream".into(),
                        retry_after: None,
                    }))
                } else {
                    Ok(dummy_stream())
                }
            }
            fn default_model(&self) -> &str {
                "test"
            }
            fn name(&self) -> &str {
                "first-fail-stream"
            }
        }

        let manager = ProviderManager::new(
            Arc::new(FirstFailStreamProvider {
                stream_count: stream_count_clone,
            }),
            None,
            None,
        );

        let result = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_ok());
        assert_eq!(stream_count.load(AtomicOrdering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_stream_circuit_open_bypasses_primary() {
        tokio::time::pause();

        let primary_stream = Arc::new(AtomicUsize::new(0));
        let fallback_stream = Arc::new(AtomicUsize::new(0));

        let manager = ProviderManager::with_config(
            Arc::new(CountingProvider::failing_streaming(
                "primary",
                primary_stream.clone(),
                auth_error,
            )),
            Some(Arc::new(CountingProvider::ok_streaming(
                "fallback",
                Arc::new(AtomicUsize::new(0)),
                fallback_stream.clone(),
            ))),
            None,
            CircuitBreakerConfig {
                failure_threshold: 2,
                reset_timeout_secs: 60,
            },
        );

        // Trip the circuit with streaming calls (2 non-retryable failures)
        let _ = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        let _ = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert_eq!(primary_stream.load(AtomicOrdering::SeqCst), 2);

        // Circuit is now open → primary skipped
        let result = manager
            .chat_stream(&[], None, &ChatParams::new("test-model"), &[])
            .await;
        assert!(result.is_ok());
        assert_eq!(primary_stream.load(AtomicOrdering::SeqCst), 2); // unchanged
        assert_eq!(fallback_stream.load(AtomicOrdering::SeqCst), 3); // 2 failover + 1 bypass
    }

    // ── Health check tests ─────────────────────────────────

    /// A provider that returns a specific health status.
    struct HealthProvider {
        health: ProviderHealth,
    }

    #[async_trait]
    impl LlmProvider for HealthProvider {
        async fn chat(
            &self,
            _: &[Message],
            _: Option<&[Value]>,
            _: &ChatParams,
            _: &[CacheBreakpoint],
        ) -> Result<LlmResponse> {
            Ok(dummy_response())
        }
        fn default_model(&self) -> &str {
            "test"
        }
        fn name(&self) -> &str {
            "health-provider"
        }
        async fn health_check(&self) -> Result<ProviderHealth> {
            Ok(self.health.clone())
        }
    }

    #[tokio::test]
    async fn test_check_health_primary_only() {
        let manager = ProviderManager::new(
            Arc::new(HealthProvider {
                health: ProviderHealth::Healthy,
            }),
            None,
            None,
        );
        let (primary, fallback) = manager.check_health().await;
        assert_eq!(primary, ProviderHealth::Healthy);
        assert!(fallback.is_none());
    }

    #[tokio::test]
    async fn test_check_health_primary_and_fallback() {
        let manager = ProviderManager::new(
            Arc::new(HealthProvider {
                health: ProviderHealth::Degraded("slow".to_string()),
            }),
            Some(Arc::new(HealthProvider {
                health: ProviderHealth::Healthy,
            })),
            None,
        );
        let (primary, fallback) = manager.check_health().await;
        assert_eq!(primary, ProviderHealth::Degraded("slow".to_string()));
        assert_eq!(fallback, Some(ProviderHealth::Healthy));
    }

    #[tokio::test]
    async fn test_check_health_unhealthy() {
        let manager = ProviderManager::new(
            Arc::new(HealthProvider {
                health: ProviderHealth::Unhealthy("down".to_string()),
            }),
            Some(Arc::new(HealthProvider {
                health: ProviderHealth::Unknown,
            })),
            None,
        );
        let (primary, fallback) = manager.check_health().await;
        assert_eq!(primary, ProviderHealth::Unhealthy("down".to_string()));
        assert_eq!(fallback, Some(ProviderHealth::Unknown));
    }

    #[tokio::test]
    async fn test_health_check_via_trait_delegates_to_primary() {
        let manager = ProviderManager::new(
            Arc::new(HealthProvider {
                health: ProviderHealth::Healthy,
            }),
            None,
            None,
        );
        let health = manager.health_check().await.unwrap();
        assert_eq!(health, ProviderHealth::Healthy);
    }

    #[tokio::test]
    async fn test_default_health_check_returns_unknown() {
        // CountingProvider doesn't override health_check, so default returns Unknown
        let provider = CountingProvider::ok("test", Arc::new(AtomicUsize::new(0)));
        let health = provider.health_check().await.unwrap();
        assert_eq!(health, ProviderHealth::Unknown);
    }
}
