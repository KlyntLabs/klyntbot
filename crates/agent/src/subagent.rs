//! Subagent manager for background task execution.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use async_trait::async_trait;
use bus::InboundMessage;
use providers::{tool_calls_to_messages, ChatParams, DynProvider, Message};
use tools::{
    filesystem::register_fs_tools,
    registry::ToolRegistry,
    shell::ExecTool,
    spawn::SpawnHandler,
    web::{WebFetchTool, WebSearchTool},
    RoutingContext,
};

/// Configuration for subagent execution
struct SubagentConfig {
    brave_api_key: Option<String>,
    web_max_results: u8,
    exec_timeout: u64,
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
    exec_timeout: u64,
    restrict_to_workspace: bool,
}

/// Builder for SubagentManager
pub struct SubagentManagerBuilder {
    provider: DynProvider,
    workspace: PathBuf,
    inbound_tx: Option<mpsc::Sender<InboundMessage>>,
    model: Option<String>,
    brave_api_key: Option<String>,
    web_max_results: u8,
    exec_timeout: u64,
    restrict_to_workspace: bool,
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
            exec_timeout: 60,
            restrict_to_workspace: true,
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

    pub fn exec_timeout(mut self, timeout: u64) -> Self {
        self.exec_timeout = timeout;
        self
    }

    pub fn restrict_to_workspace(mut self, restrict: bool) -> Self {
        self.restrict_to_workspace = restrict;
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
            exec_timeout: self.exec_timeout,
            restrict_to_workspace: self.restrict_to_workspace,
        }
    }
}

impl SubagentManager {
    /// Create a new subagent manager builder
    pub fn builder(provider: DynProvider, workspace: PathBuf) -> SubagentManagerBuilder {
        SubagentManagerBuilder::new(provider, workspace)
    }

    /// Create a new subagent manager (deprecated - use builder)
    #[deprecated(note = "Use SubagentManager::builder() instead")]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: DynProvider,
        workspace: PathBuf,
        inbound_tx: mpsc::Sender<InboundMessage>,
        model: String,
        brave_api_key: Option<String>,
        web_max_results: u8,
        exec_timeout: u64,
        restrict_to_workspace: bool,
    ) -> Self {
        Self {
            provider,
            workspace,
            inbound_tx,
            model,
            brave_api_key,
            web_max_results,
            exec_timeout,
            restrict_to_workspace,
        }
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
            exec_timeout: self.exec_timeout,
            restrict_to_workspace: self.restrict_to_workspace,
        };

        tokio::spawn(async move {
            info!("Subagent {} started: {}", subagent_id_clone, label_clone);

            let result = run_subagent_task(&provider, &workspace, &model, &task, config).await;

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
        origin_channel: String,
        origin_chat_id: String,
    ) -> String {
        self.spawn(task, label, origin_channel, origin_chat_id)
            .await
    }
}

/// Run an agent loop for a subagent task with limited tools.
///
/// Subagents have access to filesystem, shell, and web tools, but NOT message, spawn, or cron tools.
/// They run for a maximum of 15 iterations (vs 20 for main agent).
async fn run_subagent_task(
    provider: &DynProvider,
    workspace: &std::path::Path,
    model: &str,
    task: &str,
    config: SubagentConfig,
) -> std::result::Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    // Build subagent tool registry with limited tools
    let mut tools = ToolRegistry::new();

    let allowed_dir = if config.restrict_to_workspace {
        Some(workspace.to_path_buf())
    } else {
        None
    };

    // Filesystem tools
    register_fs_tools(&mut tools, allowed_dir);

    // Shell tool
    tools.register(ExecTool::new(
        config.exec_timeout,
        Some(workspace.to_path_buf()),
        config.restrict_to_workspace,
    ));

    // Web tools
    tools.register(WebSearchTool::new(
        config.brave_api_key,
        config.web_max_results,
    ));
    tools.register(WebFetchTool::new());

    // Build system prompt
    let system_prompt = build_subagent_prompt(workspace, task);

    // Initialize messages
    let mut messages = vec![
        Message::system(system_prompt),
        Message::user(task.to_string()),
    ];

    // Run agent loop with 15-iteration cap
    let max_iterations = 15;
    let mut iteration = 0;
    let mut final_result: Option<String> = None;

    // Subagents don't have meaningful routing context, use placeholder
    let routing_ctx = RoutingContext::new("subagent".into(), "background".into());

    while iteration < max_iterations {
        iteration += 1;

        debug!("Subagent iteration {}/{}", iteration, max_iterations);

        // Call LLM with tools
        let tool_defs = tools.get_definitions();
        let params = ChatParams::new(model);
        let response = provider
            .chat(&messages, Some(&tool_defs), &params)
            .await
            .map_err(|e| {
                error!("Subagent LLM call failed: {}", e);
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })?;

        // Check if there are tool calls
        if !response.tool_calls.is_empty() {
            // Convert ToolCall to ToolCallMessage
            let tool_call_messages = tool_calls_to_messages(&response.tool_calls);

            // Add assistant message with tool calls (can have both content and tool_calls)
            messages.push(Message::Assistant {
                content: response.content.clone(),
                tool_calls: Some(tool_call_messages),
                reasoning_content: None,
            });

            // Execute each tool call
            for tool_call in &response.tool_calls {
                let tool_name = &tool_call.name;
                let args = &tool_call.arguments;

                debug!("Subagent executing tool: {} with args: {}", tool_name, args);

                let result = tools
                    .execute(tool_name, args.clone(), &routing_ctx)
                    .await
                    .unwrap_or_else(|e| format!("Tool error: {}", e));

                // Add tool result message
                messages.push(Message::tool(
                    tool_call.id.clone(),
                    tool_name.clone(),
                    result,
                ));
            }

            continue;
        }

        // No tool calls - this is the final response
        final_result = response.content;
        break;
    }

    if final_result.is_none() {
        final_result = Some("Task completed but no final response was generated.".to_string());
    }

    let status = "ok"; // Successful completion regardless of iteration count

    Ok((status.to_string(), final_result.unwrap()))
}

/// Build a focused system prompt for subagents
fn build_subagent_prompt(workspace: &std::path::Path, task: &str) -> String {
    format!(
        r#"# Subagent

You are a subagent spawned by the main agent to complete a specific task.

## Your Task
{}

## Rules
1. Stay focused - complete only the assigned task, nothing else
2. Your final response will be reported back to the main agent
3. Do not initiate conversations or take on side tasks
4. Be concise but informative in your findings

## What You Can Do
- Read and write files in the workspace
- Execute shell commands
- Search the web and fetch web pages
- Complete the task thoroughly

## What You Cannot Do
- Send messages directly to users (no message tool available)
- Spawn other subagents
- Access the main agent's conversation history

## Workspace
Your workspace is at: {}

When you have completed the task, provide a clear summary of your findings or actions."#,
        task,
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
