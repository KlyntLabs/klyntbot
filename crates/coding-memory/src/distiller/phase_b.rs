//! Phase B — LLM synthesis prompt construction + invocation.
//!
//! The prompt is structured so the model's output is almost always one or
//! two `record_observation` tool calls. We include the Phase-A turn trace
//! (compact JSON), the user's prompt, and the assistant's final message.

use super::{TestOutcome, TurnTrace};
use super::error::DistillerError;
use super::record_observation::{
    decode_observations, record_observation_tool_def, Observation,
};

/// Built prompt ready to hand to `ProviderManager::chat`.
#[derive(Debug, Clone)]
pub struct DistillerPrompt {
    /// System message.
    pub system: String,
    /// User message (includes the turn trace + inputs, truncated to safe bounds).
    pub user_message: String,
}

const USER_MSG_CAP: usize = 24_000;

/// Build the Phase-B prompt.
#[must_use]
pub fn build_prompt(
    user_prompt_text: &str,
    assistant_text: &str,
    trace: &TurnTrace,
    repo_id: Option<&str>,
) -> DistillerPrompt {
    let system = SYSTEM_PROMPT.to_string();

    let trace_summary = summarize_trace(trace);
    let scope = repo_id.map(|r| format!("repo:{r}")).unwrap_or_else(|| "global".into());

    let mut user = String::new();
    user.push_str(&format!("## Scope\n{scope}\n\n"));
    user.push_str("## User prompt\n");
    user.push_str(truncate(user_prompt_text, 4_000));
    user.push_str("\n\n## Assistant final message\n");
    user.push_str(truncate(assistant_text, 4_000));
    user.push_str("\n\n## Turn trace (Phase A)\n");
    user.push_str(&trace_summary);
    user.push_str(
        "\n\n## Task\nEmit zero or more record_observation(...) tool calls. Emit nothing if \
        nothing memorable happened. Do not call any other tool. Do not respond in prose.",
    );

    if user.len() > USER_MSG_CAP {
        user.truncate(USER_MSG_CAP);
        user.push_str("\n[truncated]");
    }

    DistillerPrompt { system, user_message: user }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) { end -= 1; }
        &s[..end]
    } else {
        s
    }
}

fn summarize_trace(t: &TurnTrace) -> String {
    let mut out = String::new();
    if !t.files_read.is_empty() {
        out.push_str(&format!("- filesRead: {}\n", path_list(&t.files_read)));
    }
    if !t.files_modified.is_empty() {
        let modified: Vec<String> = t.files_modified.iter()
            .map(|(p, n)| format!("{} ({}b)", p.to_string_lossy(), n))
            .collect();
        out.push_str(&format!("- filesModified: {}\n", modified.join(", ")));
    }
    if !t.commands_run.is_empty() {
        let cmds = t.commands_run.iter().take(20).cloned().collect::<Vec<_>>().join(" | ");
        out.push_str(&format!("- commandsRun: {cmds}\n"));
    }
    if !t.test_outcomes.is_empty() {
        out.push_str("- testOutcomes:\n");
        for t in &t.test_outcomes {
            out.push_str(&format!("  - {} ({}/{} passed/failed)\n",
                t.framework.clone().unwrap_or_else(|| "?".into()), t.passed, t.failed));
            let _ = TestOutcome { command: String::new(), framework: None, passed: 0, failed: 0 };
        }
    }
    if !t.errors_encountered.is_empty() {
        out.push_str(&format!("- errors: {} encountered\n", t.errors_encountered.len()));
    }
    if let Some(u) = t.token_usage {
        out.push_str(&format!("- tokenUsage: prompt={} completion={} cached={}\n",
            u.prompt, u.completion, u.cached));
    }
    if out.is_empty() { "- (no extractive signal)".into() } else { out }
}

fn path_list(paths: &[std::path::PathBuf]) -> String {
    paths.iter().take(10).map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(", ")
}

const SYSTEM_PROMPT: &str = "You are a memory distiller for a coding assistant. From this \
coding-agent turn, emit zero or more structured observations via the `record_observation` tool. \
Each observation must use one of these 5 kinds: fix_attempt, style_preference, workflow_pattern, \
repo_context, failure_pattern. Never call any other tool. Never respond in prose. Emit nothing \
if nothing significant happened. Be conservative — prefer precision over recall.";

use std::sync::Arc;
use std::time::Duration;

/// Inputs for an LLM invocation.
pub struct LlmInvocation<'a> {
    /// Shared ProviderManager (failover, retry, circuit breaker).
    pub provider: Arc<providers::ProviderManager>,
    /// Model id — from `DistillerConfig::model`.
    pub model: String,
    /// User prompt text from the turn.
    pub user_prompt_text: &'a str,
    /// Final assistant message text from the turn.
    pub assistant_text: &'a str,
    /// Phase-A trace.
    pub trace: &'a TurnTrace,
    /// Resolved repo id, if any.
    pub repo_id: Option<&'a str>,
    /// Wall-clock timeout — cancels on elapse.
    pub timeout: Duration,
}

/// Run the Phase-B LLM call. Returns the list of decoded observations (may be empty).
pub async fn invoke_llm(inv: LlmInvocation<'_>) -> Result<Vec<Observation>, DistillerError> {
    let prompt = build_prompt(inv.user_prompt_text, inv.assistant_text, inv.trace, inv.repo_id);
    let messages: Vec<providers::types::Message> = vec![
        providers::types::Message::System { content: prompt.system },
        providers::types::Message::User { content: providers::types::UserContent::Text(prompt.user_message) },
    ];
    let tools = Some(vec![record_observation_tool_def()]);
    let params = providers::types::ChatParams::new(inv.model.clone())
        .with_temperature(0.2)
        .with_max_tokens(1024);

    use providers::LlmProvider;
    let fut = inv.provider.chat(&messages, tools.as_deref(), &params);
    let resp = tokio::time::timeout(inv.timeout, fut)
        .await
        .map_err(|_| DistillerError::LlmTimeout { timeout_ms: inv.timeout.as_millis() as u64 })?
        .map_err(|e| DistillerError::LlmProvider { detail: e.to_string() })?;

    let args: Vec<serde_json::Value> = resp.tool_calls
        .into_iter()
        .filter(|tc| tc.name == super::record_observation::RECORD_OBSERVATION_TOOL_NAME)
        .map(|tc| tc.arguments)
        .collect();
    decode_observations(&args)
}
