//! Subagent manager for background task execution.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, info, warn};
use uuid::Uuid;

use async_trait::async_trait;
use bus::InboundMessage;
use providers::{DynProvider, Message};
use tools::{
    filesystem::{register_fs_read_tools, register_fs_tools},
    registry::ToolRegistry,
    spawn::SpawnHandler,
    web::{WebFetchTool, WebSearchTool},
    RoutingContext,
};

/// Specialized profiles for sub-agents with different tool sets and behaviors.
///
/// klyntbot is a personal AI agent — no code execution tools are provided.
/// Profiles control access to filesystem (read/write), web, and browser tools.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SubagentProfile {
    /// Full access: filesystem + web (default)
    #[default]
    General,
    /// Web + filesystem read-only
    Research,
    /// Filesystem read-only only (pure reasoning)
    Analyst,
}

impl FromStr for SubagentProfile {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "research" => Self::Research,
            "analyst" => Self::Analyst,
            _ => Self::General,
        })
    }
}

impl SubagentProfile {
    /// Maximum iteration count for this profile.
    pub fn max_iterations(&self) -> u32 {
        match self {
            Self::General => 15,
            Self::Research => 10,
            Self::Analyst => 5,
        }
    }

    /// System prompt role preamble for this profile.
    pub fn role_prompt(&self) -> &'static str {
        match self {
            Self::General => "You are a general-purpose subagent. You have full access to filesystem and web tools.",
            Self::Research => "You are a research specialist. Focus on finding and synthesizing information from web sources and files. You have read-only file access and web tools.",
            Self::Analyst => "You are an analyst. Reason about the information provided. You have read-only file access but no web tools.",
        }
    }
}

/// Configuration for subagent execution
struct SubagentConfig {
    brave_api_key: Option<String>,
    web_max_results: u8,
    /// Timeout in seconds for the subagent task.
    task_timeout: u64,
    restrict_to_workspace: bool,
}

/// Subagent manager for spawning background tasks
pub struct SubagentManager {
    provider: DynProvider,
    workspace: PathBuf,
    inbound_tx: mpsc::Sender<InboundMessage>,
    model: String,
    brave_api_key: Option<String>,
    web_max_results: u8,
    task_timeout: u64,
    restrict_to_workspace: bool,
    semaphore: Arc<Semaphore>,
}

/// Builder for SubagentManager
pub struct SubagentManagerBuilder {
    provider: DynProvider,
    workspace: PathBuf,
    inbound_tx: Option<mpsc::Sender<InboundMessage>>,
    model: Option<String>,
    brave_api_key: Option<String>,
    web_max_results: u8,
    task_timeout: u64,
    restrict_to_workspace: bool,
    max_concurrent_subagents: usize,
}

impl SubagentManagerBuilder {
    /// Create a new builder with required parameters
    pub fn new(provider: DynProvider, workspace: PathBuf) -> Self {
        Self {
            provider,
            workspace,
            inbound_tx: None,
            model: None,
            brave_api_key: None,
            web_max_results: 5,
            task_timeout: 60,
            restrict_to_workspace: true,
            max_concurrent_subagents: 3,
        }
    }

    pub fn inbound_sender(mut self, tx: mpsc::Sender<InboundMessage>) -> Self {
        self.inbound_tx = Some(tx);
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn brave_api_key(mut self, key: Option<String>) -> Self {
        self.brave_api_key = key;
        self
    }

    pub fn web_max_results(mut self, max: u8) -> Self {
        self.web_max_results = max;
        self
    }

    pub fn task_timeout(mut self, timeout: u64) -> Self {
        self.task_timeout = timeout;
        self
    }

    pub fn restrict_to_workspace(mut self, restrict: bool) -> Self {
        self.restrict_to_workspace = restrict;
        self
    }

    pub fn max_concurrent_subagents(mut self, n: usize) -> Self {
        self.max_concurrent_subagents = n;
        self
    }

    pub fn build(self) -> SubagentManager {
        SubagentManager {
            provider: self.provider,
            workspace: self.workspace,
            inbound_tx: self.inbound_tx.expect("inbound_sender is required"),
            model: self.model.unwrap_or_else(|| "claude-sonnet-4".to_string()),
            brave_api_key: self.brave_api_key,
            web_max_results: self.web_max_results,
            task_timeout: self.task_timeout,
            restrict_to_workspace: self.restrict_to_workspace,
            semaphore: Arc::new(Semaphore::new(self.max_concurrent_subagents)),
        }
    }
}

impl SubagentManager {
    /// Create a new subagent manager builder
    pub fn builder(provider: DynProvider, workspace: PathBuf) -> SubagentManagerBuilder {
        SubagentManagerBuilder::new(provider, workspace)
    }

    /// Returns the total semaphore permit count (max concurrent subagents).
    pub fn semaphore_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Spawn a subagent for a background task.
    ///
    /// The subagent runs a simplified agent loop: it sends the task as a user message
    /// to the LLM with read-only context, then reports the result back to the main
    /// agent via the inbound bus as a system message.
    pub async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        profile: SubagentProfile,
        origin_channel: String,
        origin_chat_id: String,
    ) -> String {
        let subagent_id = Uuid::new_v4().to_string();
        let label_text = label.unwrap_or_else(|| "background task".to_string());

        debug!("Spawning subagent {} for task: {}", subagent_id, task);

        let provider = Arc::clone(&self.provider);
        let workspace = self.workspace.clone();
        let inbound_tx = self.inbound_tx.clone();
        let model = self.model.clone();
        let origin_key = format!("{}:{}", origin_channel, origin_chat_id);
        let subagent_id_clone = subagent_id.clone();
        let label_clone = label_text.clone();
        let config = SubagentConfig {
            brave_api_key: self.brave_api_key.clone(),
            web_max_results: self.web_max_results,
            task_timeout: self.task_timeout,
            restrict_to_workspace: self.restrict_to_workspace,
        };

        let semaphore = Arc::clone(&self.semaphore);

        tokio::spawn(async move {
            // Acquire permit before running — limits concurrent subagents.
            // _permit is held for the duration of the task, then dropped automatically.
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("Semaphore closed unexpectedly");

            info!("Subagent {} started: {}", subagent_id_clone, label_clone);

            let result =
                run_subagent_task(&provider, &workspace, &model, &task, config, profile).await;

            match result {
                Ok((status, result_text)) => {
                    announce_result(
                        &inbound_tx,
                        &subagent_id_clone,
                        &label_clone,
                        &task,
                        &result_text,
                        &origin_key,
                        &status,
                    )
                    .await;
                }
                Err(e) => {
                    let error_msg = format!("Error: {}", e);
                    announce_result(
                        &inbound_tx,
                        &subagent_id_clone,
                        &label_clone,
                        &task,
                        &error_msg,
                        &origin_key,
                        "error",
                    )
                    .await;
                }
            }

            info!("Subagent {} completed", subagent_id_clone);
        });

        format!(
            "Subagent spawned (ID: {}). Working on '{}' in the background...",
            &subagent_id[..8],
            label_text
        )
    }
}

/// Implement SpawnHandler trait for dependency inversion (tools layer can use agent layer)
#[async_trait]
impl SpawnHandler for SubagentManager {
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        profile: String,
        origin_channel: String,
        origin_chat_id: String,
    ) -> String {
        let profile = SubagentProfile::from_str(&profile).unwrap_or_default();
        self.spawn(task, label, profile, origin_channel, origin_chat_id)
            .await
    }
}

/// Run an agent loop for a subagent task with profile-based tools.
///
/// Tool access is determined by `SubagentProfile`:
/// - General: filesystem + web (full access)
/// - Research: read-only filesystem + web (no writes)
/// - Analyst: read-only filesystem only (pure reasoning)
async fn run_subagent_task(
    provider: &DynProvider,
    workspace: &std::path::Path,
    model: &str,
    task: &str,
    config: SubagentConfig,
    profile: SubagentProfile,
) -> std::result::Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    use crate::execution::core::ExecutionCore;
    use crate::execution::react_plus::{ReactOutcome, ReactPlusEngine, ReflectionMode};
    use crate::execution::types::ExecutionParams;
    use tokio::sync::RwLock;

    // Build subagent tool registry based on profile
    let mut tools = ToolRegistry::new();

    let allowed_dir = if config.restrict_to_workspace {
        Some(workspace.to_path_buf())
    } else {
        None
    };

    match profile {
        SubagentProfile::General => {
            register_fs_tools(&mut tools, allowed_dir);
            tools.register(WebSearchTool::new(
                config.brave_api_key,
                config.web_max_results,
            ));
            tools.register(WebFetchTool::new());
        }
        SubagentProfile::Research => {
            register_fs_read_tools(&mut tools, allowed_dir);
            tools.register(WebSearchTool::new(
                config.brave_api_key,
                config.web_max_results,
            ));
            tools.register(WebFetchTool::new());
        }
        SubagentProfile::Analyst => {
            register_fs_read_tools(&mut tools, allowed_dir);
        }
    }

    let tool_defs = tools.get_definitions();

    // Build execution engine
    let core = Arc::new(ExecutionCore::new(
        Arc::clone(provider),
        Arc::new(RwLock::new(tools)),
    ));
    let engine = ReactPlusEngine::new(core)
        .with_max_iterations(profile.max_iterations())
        .with_reflection_mode(ReflectionMode::OnFailure);

    // Build system prompt and messages
    let system_prompt = build_subagent_prompt(workspace, task, profile);
    let messages = vec![
        Message::system(system_prompt),
        Message::user(task.to_string()),
    ];

    let params = ExecutionParams::new(model)
        .with_timeout(std::time::Duration::from_secs(config.task_timeout));
    let routing_ctx = RoutingContext::new("subagent".into(), "background".into());

    // Execute via ReactPlusEngine
    let outcome = engine
        .execute(Arc::new(messages), &tool_defs, &params, &routing_ctx, None)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    match outcome {
        ReactOutcome::Response { content, .. } => Ok(("ok".to_string(), content)),
        ReactOutcome::EscalateToAutonomous { reason, .. } => Ok((
            "ok".to_string(),
            format!("Task requires more complex handling: {}", reason),
        )),
        ReactOutcome::MaxIterationsReached {
            partial_content, ..
        } => {
            let text = partial_content.unwrap_or_else(|| {
                "Task completed but no final response was generated.".to_string()
            });
            Ok(("ok".to_string(), text))
        }
    }
}

/// Build a focused system prompt for subagents, tailored to the profile.
fn build_subagent_prompt(
    workspace: &std::path::Path,
    task: &str,
    profile: SubagentProfile,
) -> String {
    let tool_description = match profile {
        SubagentProfile::General => "- Read and write files in the workspace\n- Search the web and fetch web pages",
        SubagentProfile::Research => "- Read files in the workspace (read-only)\n- Search the web and fetch web pages",
        SubagentProfile::Analyst => "- Read files in the workspace (read-only)",
    };

    format!(
        r#"# Subagent

{}

## Your Task
{}

## Rules
1. Stay focused - complete only the assigned task, nothing else
2. Your final response will be reported back to the main agent
3. Do not initiate conversations or take on side tasks
4. Be concise but informative in your findings

## What You Can Do
{}

## What You Cannot Do
- Send messages directly to users (no message tool available)
- Spawn other subagents
- Access the main agent's conversation history

## Workspace
Your workspace is at: {}

When you have completed the task, provide a clear summary of your findings or actions."#,
        profile.role_prompt(),
        task,
        tool_description,
        workspace.display()
    )
}

/// Announce subagent result back to main agent via message bus
async fn announce_result(
    inbound_tx: &mpsc::Sender<InboundMessage>,
    task_id: &str,
    label: &str,
    task: &str,
    result: &str,
    origin_key: &str,
    status: &str,
) {
    let status_text = if status == "ok" {
        "completed successfully"
    } else {
        "failed"
    };

    let announce_content = format!(
        r#"[Subagent '{}' {}]

Task: {}

Result:
{}

Summarize this naturally for the user. Keep it brief (1-2 sentences). Do not mention technical details like "subagent" or task IDs."#,
        label, status_text, task, result
    );

    let msg = InboundMessage::new("system", "subagent", origin_key, announce_content);

    if let Err(e) = inbound_tx.send(msg).await {
        warn!("Failed to send subagent result: {}", e);
    }

    debug!("Subagent [{}] announced result to {}", task_id, origin_key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use common::Result;
    use providers::{ChatParams, LlmProvider, LlmResponse, Message, Usage};
    use tokio::sync::mpsc;

    /// Minimal no-op provider for unit tests — never actually called.
    struct NoOpProvider;

    #[async_trait]
    impl LlmProvider for NoOpProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some("ok".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }

        fn default_model(&self) -> &str {
            "no-op"
        }

        fn name(&self) -> &str {
            "no-op"
        }
    }

    fn make_manager(permits: usize) -> SubagentManager {
        let (tx, _rx) = mpsc::channel(8);
        let provider: DynProvider = Arc::new(NoOpProvider);
        SubagentManager::builder(provider, std::path::PathBuf::from("/tmp"))
            .inbound_sender(tx)
            .max_concurrent_subagents(permits)
            .build()
    }

    #[test]
    fn test_subagent_manager_has_semaphore() {
        let mgr = make_manager(5);
        assert_eq!(mgr.semaphore_permits(), 5);
    }

    #[test]
    fn test_default_semaphore_permits_match_config_default() {
        let mgr = make_manager(3);
        assert_eq!(mgr.semaphore_permits(), 3);
    }

    #[tokio::test]
    async fn test_run_subagent_task_returns_text_response() {
        let provider: DynProvider = Arc::new(NoOpProvider);
        let config = SubagentConfig {
            brave_api_key: None,
            web_max_results: 5,
            task_timeout: 60,
            restrict_to_workspace: false,
        };
        let result = run_subagent_task(
            &provider,
            std::path::Path::new("/tmp"),
            "no-op",
            "Say hello",
            config,
            SubagentProfile::General,
        )
        .await;
        assert!(result.is_ok());
        let (status, text) = result.unwrap();
        assert_eq!(status, "ok");
        assert_eq!(text, "ok"); // NoOpProvider returns "ok"
    }

    #[tokio::test]
    async fn test_semaphore_limits_concurrency() {
        // Verify that the semaphore correctly limits concurrent acquisitions.
        let sem = Arc::new(Semaphore::new(2));

        let p1 = Arc::clone(&sem).acquire_owned().await.unwrap();
        let p2 = Arc::clone(&sem).acquire_owned().await.unwrap();

        // All permits are held — further tries should fail immediately.
        assert!(sem.try_acquire().is_err());

        // Release one permit; now one slot is free.
        drop(p1);
        let p3 = sem.try_acquire();
        assert!(p3.is_ok());

        drop(p2);
    }

    #[test]
    fn test_subagent_profile_default_is_general() {
        let profile = SubagentProfile::default();
        assert!(matches!(profile, SubagentProfile::General));
    }

    #[test]
    fn test_subagent_profile_from_str() {
        assert!(matches!(
            SubagentProfile::from_str("research"),
            Ok(SubagentProfile::Research)
        ));
        assert!(matches!(
            SubagentProfile::from_str("analyst"),
            Ok(SubagentProfile::Analyst)
        ));
        assert!(matches!(
            SubagentProfile::from_str("general"),
            Ok(SubagentProfile::General)
        ));
        // Unknown profiles default to General
        assert!(matches!(
            SubagentProfile::from_str("code"),
            Ok(SubagentProfile::General)
        ));
        assert!(matches!(
            SubagentProfile::from_str("unknown"),
            Ok(SubagentProfile::General)
        ));
    }

    #[test]
    fn test_subagent_profile_max_iterations() {
        assert_eq!(SubagentProfile::General.max_iterations(), 15);
        assert_eq!(SubagentProfile::Research.max_iterations(), 10);
        assert_eq!(SubagentProfile::Analyst.max_iterations(), 5);
    }

    #[test]
    fn test_build_subagent_prompt_includes_profile_role() {
        let prompt = build_subagent_prompt(
            std::path::Path::new("/tmp"),
            "Research Rust patterns",
            SubagentProfile::Research,
        );
        assert!(prompt.contains("research specialist"));
        assert!(prompt.contains("Research Rust patterns"));
        assert!(prompt.contains("read-only"));
    }

    #[test]
    fn test_build_subagent_prompt_general_profile() {
        let prompt = build_subagent_prompt(
            std::path::Path::new("/tmp"),
            "Do something",
            SubagentProfile::General,
        );
        assert!(prompt.contains("general-purpose subagent"));
        assert!(prompt.contains("Search the web"));
    }

    #[test]
    fn test_build_subagent_prompt_analyst_profile() {
        let prompt = build_subagent_prompt(
            std::path::Path::new("/tmp"),
            "Analyze data",
            SubagentProfile::Analyst,
        );
        assert!(prompt.contains("analyst"));
        assert!(prompt.contains("read-only"));
        // "What You Can Do" section should NOT list shell capabilities
        assert!(!prompt.contains("Execute shell commands"));
    }
}
