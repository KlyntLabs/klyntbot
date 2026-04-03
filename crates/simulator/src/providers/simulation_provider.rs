//! A topic-aware LLM provider that returns tool-call JSON for simulation.
//!
//! Inspects the user message for topic keywords and returns structured
//! tool calls that drive the ReAct loop through real tool execution.
//! For messages without clear tool intent, returns plain text (Direct mode).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::{json, Value};

use common::Result;
use providers::types::{
    ChatParams, DynProvider, LlmProvider, LlmResponse, Message, ProviderCapabilities,
    ProviderHealth, ToolCall, Usage, UserContent,
};

pub struct SimulationProvider {
    call_count: AtomicUsize,
    rng: Mutex<StdRng>,
}

impl SimulationProvider {
    pub fn new(seed: u64) -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            rng: Mutex::new(StdRng::seed_from_u64(seed)),
        }
    }

    /// Extract text content from the last user message.
    fn last_user_content(messages: &[Message]) -> Option<String> {
        messages.iter().rev().find_map(|m| match m {
            Message::User { content } => match content {
                UserContent::Text(s) => Some(s.clone()),
                UserContent::MultiPart(parts) => {
                    // Extract text from first text part
                    parts.iter().find_map(|p| match p {
                        providers::types::ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                }
            },
            _ => None,
        })
    }

    /// Check for tool results from previous iterations and generate follow-up tool calls.
    fn generate_chained_call(&self, messages: &[Message]) -> Option<Vec<ToolCall>> {
        // Look for Tool result messages (from previous reactive iterations)
        let last_tool = messages.iter().rev().find_map(|m| match m {
            Message::Tool { name, .. } => Some(name.as_str()),
            _ => None,
        })?;

        let call_id = self.call_count.load(Ordering::Relaxed);

        match last_tool {
            "notes" => Some(vec![ToolCall {
                id: format!("call_{call_id}_chain"),
                name: "tasks".to_string(),
                arguments: json!({"action": "create", "title": "Follow up on note", "project": "main"}),
            }]),
            "tasks" => Some(vec![ToolCall {
                id: format!("call_{call_id}_chain"),
                name: "notes".to_string(),
                arguments: json!({"action": "search", "query": "task summary"}),
            }]),
            "finance" => Some(vec![ToolCall {
                id: format!("call_{call_id}_chain"),
                name: "notes".to_string(),
                arguments: json!({"action": "search", "query": "financial summary"}),
            }]),
            _ => None,
        }
    }

    /// Inspect the last user message and return appropriate tool calls.
    fn generate_tool_calls(&self, messages: &[Message]) -> Option<Vec<ToolCall>> {
        let content = Self::last_user_content(messages)?;
        let lower = content.to_lowercase();

        // Multi-domain detection: check for 2+ domain keywords
        let has_task = lower.contains("task") || lower.contains("todo");
        let has_note = lower.contains("note") || lower.contains("summarize");
        let has_finance =
            lower.contains("expense") || lower.contains("budget") || lower.contains("spend");
        let has_focus = lower.contains("focus") || lower.contains("productive");

        let domain_count = [has_task, has_note, has_finance, has_focus]
            .iter()
            .filter(|&&b| b)
            .count();

        if domain_count >= 2 {
            let call_id = self.call_count.load(Ordering::Relaxed);
            let mut calls = Vec::new();
            if has_task {
                calls.push(ToolCall {
                    id: format!("call_{call_id}_tasks"),
                    name: "tasks".to_string(),
                    arguments: json!({"action": "list"}),
                });
            }
            if has_note {
                calls.push(ToolCall {
                    id: format!("call_{call_id}_notes"),
                    name: "notes".to_string(),
                    arguments: json!({"action": "search", "query": content}),
                });
            }
            if has_finance {
                calls.push(ToolCall {
                    id: format!("call_{call_id}_finance"),
                    name: "finance".to_string(),
                    arguments: json!({"action": "record", "amount": 50.0, "category": "general", "description": "Simulated"}),
                });
            }
            if has_focus {
                calls.push(ToolCall {
                    id: format!("call_{call_id}_productivity"),
                    name: "productivity".to_string(),
                    arguments: json!({"action": "start_focus", "duration_mins": 25}),
                });
            }
            return Some(calls);
        }

        // Task-related
        if lower.contains("task") || lower.contains("todo") || lower.contains("prioritize") {
            if lower.contains("done") || lower.contains("complete") || lower.contains("mark") {
                return Some(vec![ToolCall {
                    id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                    name: "tasks".to_string(),
                    arguments: json!({"action": "list"}),
                }]);
            }
            if lower.contains("create") || lower.contains("add") || lower.contains("need to") {
                return Some(vec![ToolCall {
                    id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                    name: "tasks".to_string(),
                    arguments: json!({
                        "action": "create",
                        "title": "Simulated task",
                        "project": "main"
                    }),
                }]);
            }
            return Some(vec![ToolCall {
                id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                name: "tasks".to_string(),
                arguments: json!({"action": "list"}),
            }]);
        }

        // Finance
        if lower.contains("expense")
            || lower.contains("budget")
            || lower.contains("spend")
            || lower.contains("income")
        {
            return Some(vec![ToolCall {
                id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                name: "finance".to_string(),
                arguments: json!({
                    "action": "record",
                    "amount": 50.0,
                    "category": "general",
                    "description": "Simulated expense"
                }),
            }]);
        }

        // Notes
        if lower.contains("note") || lower.contains("summarize") || lower.contains("write") {
            return Some(vec![ToolCall {
                id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                name: "notes".to_string(),
                arguments: json!({"action": "search", "query": content}),
            }]);
        }

        // Productivity
        if lower.contains("focus") || lower.contains("productive") || lower.contains("time") {
            return Some(vec![ToolCall {
                id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                name: "productivity".to_string(),
                arguments: json!({"action": "start_focus", "duration_mins": 25}),
            }]);
        }

        // Learning
        if lower.contains("learn") || lower.contains("flashcard") || lower.contains("quiz") {
            return Some(vec![ToolCall {
                id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                name: "learning".to_string(),
                arguments: json!({
                    "action": "create_flashcard",
                    "front": "What is this concept?",
                    "back": "A key concept"
                }),
            }]);
        }

        // Automation
        if lower.contains("remind") || lower.contains("recurring") || lower.contains("automate") {
            return Some(vec![ToolCall {
                id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                name: "cron".to_string(),
                arguments: json!({"action": "list"}),
            }]);
        }

        // Insights / work context
        if lower.contains("pattern") || lower.contains("connection") || lower.contains("insight") {
            return Some(vec![ToolCall {
                id: format!("call_{}", self.call_count.load(Ordering::Relaxed)),
                name: "work_context".to_string(),
                arguments: json!({"action": "query"}),
            }]);
        }

        // No tool match — return None for plain text response (Direct mode)
        None
    }
}

#[async_trait]
impl LlmProvider for SimulationProvider {
    async fn chat(
        &self,
        messages: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
    ) -> Result<LlmResponse> {
        let _idx = self.call_count.fetch_add(1, Ordering::SeqCst);

        let (prompt_tokens, completion_tokens) = {
            let mut rng = self.rng.lock().unwrap();
            (rng.random_range(80..200u32), rng.random_range(30..120u32))
        };

        // Check for sequential chaining (tool results from previous iterations)
        if let Some(chained) = self.generate_chained_call(messages) {
            return Ok(LlmResponse {
                content: None,
                tool_calls: chained,
                finish_reason: "tool_use".to_string(),
                usage: Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                },
                reasoning_content: None,
            });
        }

        let tool_calls = self.generate_tool_calls(messages).unwrap_or_default();
        let has_tools = !tool_calls.is_empty();
        let response_content = if has_tools {
            None
        } else {
            Some("I understand. Let me help you with that.".to_string())
        };

        Ok(LlmResponse {
            content: response_content,
            tool_calls,
            finish_reason: if has_tools {
                "tool_use".to_string()
            } else {
                "stop".to_string()
            },
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            reasoning_content: None,
        })
    }

    // NOTE: chat_stream intentionally not overridden — the default impl in
    // LlmProvider wraps chat() into a single-chunk stream, which is what the
    // AgentRuntime's ExecutionCore expects when an event channel is provided.

    fn supports_streaming(&self) -> bool {
        false
    }

    fn default_model(&self) -> &str {
        "simulation-agent"
    }

    fn name(&self) -> &str {
        "simulation-provider"
    }

    async fn count_tokens(&self, _messages: &[Message], _tools: Option<&[Value]>) -> Result<usize> {
        let mut rng = self.rng.lock().unwrap();
        Ok(rng.random_range(100..250usize))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            extended_thinking: false,
            structured_outputs: false,
            prompt_caching: false,
            native_token_counting: false,
            vision: false,
            streaming: false,
            tool_choice_required: false,
            parallel_tool_calls: false,
        }
    }

    fn context_window(&self) -> usize {
        128_000
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth::Healthy)
    }

    fn classifier_provider(&self) -> Option<DynProvider> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_task_tool_call_for_task_message() {
        let provider = SimulationProvider::new(42);
        let messages = vec![Message::user("Create a task: review PR for main project")];
        let params = ChatParams::new("simulation-agent");

        let response = provider.chat(&messages, None, &params).await.unwrap();

        assert!(!response.tool_calls.is_empty(), "should return tool calls");
        assert_eq!(response.tool_calls[0].name, "tasks");
        assert!(
            response.content.is_none(),
            "tool call response should have no text content"
        );
        assert_eq!(response.finish_reason, "tool_use");
    }

    #[tokio::test]
    async fn returns_plain_text_for_chat_message() {
        let provider = SimulationProvider::new(42);
        let messages = vec![Message::user("Good morning")];
        let params = ChatParams::new("simulation-agent");

        let response = provider.chat(&messages, None, &params).await.unwrap();

        assert!(
            response.tool_calls.is_empty(),
            "chat should not trigger tool calls"
        );
        assert!(
            response.content.is_some(),
            "chat should return text content"
        );
        assert_eq!(response.finish_reason, "stop");
    }

    #[tokio::test]
    async fn returns_finance_tool_call() {
        let provider = SimulationProvider::new(42);
        let messages = vec![Message::user("Record expense: $50 for lunch")];
        let params = ChatParams::new("simulation-agent");

        let response = provider.chat(&messages, None, &params).await.unwrap();

        assert_eq!(response.tool_calls[0].name, "finance");
    }
}
