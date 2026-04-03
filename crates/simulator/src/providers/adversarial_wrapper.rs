//! Wraps any LlmProvider to probabilistically inject malformed responses.
//! Used for adversarial testing — the inner provider can be mock or real LLM.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::{json, Value};

use common::Result;
use providers::types::{
    ChatParams, DynProvider, LlmProvider, LlmResponse, Message, ProviderCapabilities,
    ProviderHealth, ToolCall, Usage,
};

pub struct AdversarialProviderWrapper {
    inner: DynProvider,
    error_rate: f64,
    rng: Mutex<StdRng>,
    inject_count: AtomicUsize,
}

impl AdversarialProviderWrapper {
    pub fn new(inner: DynProvider, error_rate: f64, seed: u64) -> Self {
        Self {
            inner,
            error_rate,
            rng: Mutex::new(StdRng::seed_from_u64(seed.wrapping_add(777))),
            inject_count: AtomicUsize::new(0),
        }
    }

    /// Check injection probability and generate a malformed response in one lock.
    fn try_inject(&self) -> Option<LlmResponse> {
        if self.error_rate <= 0.0 {
            return None;
        }
        let malformation = {
            let mut rng = self.rng.lock().unwrap();
            if rng.random::<f64>() >= self.error_rate {
                return None;
            }
            rng.random_range(0u8..4)
        };
        self.inject_count.fetch_add(1, Ordering::Relaxed);
        let bad_call = match malformation {
            0 => ToolCall {
                id: "adversarial_inject".to_string(),
                name: "taks".to_string(), // typo
                arguments: json!({"action": "list"}),
            },
            1 => ToolCall {
                id: "adversarial_inject".to_string(),
                name: "tasks".to_string(),
                arguments: json!(null), // invalid arguments
            },
            2 => ToolCall {
                id: String::new(), // empty ID
                name: "tasks".to_string(),
                arguments: json!({"action": "list"}),
            },
            _ => ToolCall {
                id: "adversarial_inject".to_string(),
                name: "nonexistent_tool".to_string(),
                arguments: json!({"action": "query"}),
            },
        };
        Some(LlmResponse {
            content: None,
            tool_calls: vec![bad_call],
            finish_reason: "tool_use".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        })
    }
}

#[async_trait]
impl LlmProvider for AdversarialProviderWrapper {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmResponse> {
        if let Some(injected) = self.try_inject() {
            return Ok(injected);
        }
        self.inner.chat(messages, tools, params).await
    }

    fn supports_streaming(&self) -> bool {
        self.inner.supports_streaming()
    }

    fn default_model(&self) -> &str {
        self.inner.default_model()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn context_window(&self) -> usize {
        self.inner.context_window()
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        self.inner.health_check().await
    }

    fn classifier_provider(&self) -> Option<DynProvider> {
        self.inner.classifier_provider()
    }
}
