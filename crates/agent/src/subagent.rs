//! Subagent manager for background task execution.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use async_trait::async_trait;
use bus::InboundMessage;
use providers::{DynProvider, Message};
use storage::AgentTaskRepo;
use tools::{
    agent_task_tool::AgentTaskTool, registry::ToolRegistry, spawn::SpawnHandler, RoutingContext,
};

/// Specialized profiles for sub-agents with different tool sets and behaviors.
///
/// klyntbot is a personal AI agent — no code execution tools are provided.
/// Profiles control access to filesystem (read/write), web, and browser tools.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SubagentProfile {
    /// Read-only access: search, read files, web fetch, ask user (default)
    #[default]
    ReadOnly,
    /// Read-write access: read-only tools + bash, write, edit, apply_patch, notebook_edit
    ReadWrite,
    /// Full access: all read-write tools + plan-mode tools
    Full,
}

impl FromStr for SubagentProfile {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "read_only" | "readonly" | "analyst" | "research" => Self::ReadOnly,
            "read_write" | "readwrite" | "general" => Self::ReadWrite,
            "full" | "code" => Self::Full,
            _ => Self::ReadWrite,
        })
    }
}

impl std::fmt::Display for SubagentProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => f.write_str("read_only"),
            Self::ReadWrite => f.write_str("read_write"),
            Self::Full => f.write_str("full"),
        }
    }
}

impl SubagentProfile {
    /// Maximum iteration count for this profile.
    pub fn max_iterations(&self) -> u32 {
        match self {
            Self::ReadOnly => 5,
            Self::ReadWrite => 10,
            Self::Full => 15,
        }
    }

    /// System prompt role preamble for this profile.
    pub fn role_prompt(&self) -> &'static str {
        match self {
            Self::ReadOnly => "You are an analyst. Reason about the information provided. You have read-only file access and web fetch tools.",
            Self::ReadWrite => "You are a research specialist. Focus on finding and synthesizing information from files. You have read and write file access.",
            Self::Full => "You are a general-purpose subagent. You have full access to filesystem, web, and plan-mode tools.",
        }
    }

    /// Tool description for the system prompt.
    pub fn tool_description(&self) -> &'static str {
        match self {
            Self::ReadOnly => "- Read files in the workspace (read-only)\n- Search file contents (grep) and find files by pattern (glob)\n- Fetch web pages",
            Self::ReadWrite => "- Read and write files in the workspace\n- Search file contents (grep) and find files by pattern (glob)\n- Execute shell commands and apply patches",
            Self::Full => "- Read and write files in the workspace\n- Search file contents (grep) and find files by pattern (glob)\n- Execute shell commands, apply patches, and manage plan mode",
        }
    }
}

/// Tracks a running subagent for cancel/status operations.
struct SubagentHandle {
    cancel_token: CancellationToken,
    label: String,
    profile: SubagentProfile,
    spawned_at: std::time::Instant,
}

/// Configuration for subagent execution
struct SubagentConfig {
    /// Timeout in seconds for the subagent task.
    task_timeout: u64,
    agent_task_repo: AgentTaskRepo,
    session_key: String,
    agent_id: String,
}

/// Subagent manager for spawning background tasks
pub struct SubagentManager {
    provider: DynProvider,
    workspace: PathBuf,
    inbound_tx: mpsc::Sender<InboundMessage>,
    model: String,
    task_timeout: u64,
    semaphore: Arc<Semaphore>,
    handles: Arc<Mutex<HashMap<String, SubagentHandle>>>,
    agent_task_repo: AgentTaskRepo,
    tool_kit: std::sync::Mutex<Option<Arc<klynt_core::ToolKitBuilder>>>,
    hook_engine: std::sync::Mutex<Option<Arc<klynt_hooks::HookEngine>>>,
    /// Cache of base ToolRegistry builds keyed by (profile, cwd).
    /// The cache stores the registry *before* AgentTaskTool is added,
    /// so each invocation can clone the base and append its task tool.
    registry_cache: std::sync::Mutex<HashMap<(SubagentProfile, PathBuf), Arc<RwLock<ToolRegistry>>>>,
}

/// Builder for SubagentManager
pub struct SubagentManagerBuilder {
    provider: DynProvider,
    workspace: PathBuf,
    inbound_tx: Option<mpsc::Sender<InboundMessage>>,
    model: Option<String>,
    task_timeout: u64,
    max_concurrent_subagents: usize,
    agent_task_repo: Option<AgentTaskRepo>,
    tool_kit: Option<Arc<klynt_core::ToolKitBuilder>>,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
}

impl SubagentManagerBuilder {
    /// Create a new builder with required parameters
    pub fn new(provider: DynProvider, workspace: PathBuf) -> Self {
        Self {
            provider,
            workspace,
            inbound_tx: None,
            model: None,
            task_timeout: 60,
            max_concurrent_subagents: 3,
            agent_task_repo: None,
            tool_kit: None,
            hook_engine: None,
        }
    }

    pub fn tool_kit(mut self, kit: Arc<klynt_core::ToolKitBuilder>) -> Self {
        self.tool_kit = Some(kit);
        self
    }

    pub fn hook_engine(mut self, engine: Arc<klynt_hooks::HookEngine>) -> Self {
        self.hook_engine = Some(engine);
        self
    }

    pub fn inbound_sender(mut self, tx: mpsc::Sender<InboundMessage>) -> Self {
        self.inbound_tx = Some(tx);
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn task_timeout(mut self, timeout: u64) -> Self {
        self.task_timeout = timeout;
        self
    }

    pub fn max_concurrent_subagents(mut self, n: usize) -> Self {
        self.max_concurrent_subagents = n;
        self
    }

    pub fn agent_task_repo(mut self, repo: AgentTaskRepo) -> Self {
        self.agent_task_repo = Some(repo);
        self
    }

    pub fn build(self) -> SubagentManager {
        SubagentManager {
            provider: self.provider,
            workspace: self.workspace,
            inbound_tx: self.inbound_tx.expect("inbound_sender is required"),
            model: self.model.unwrap_or_else(|| "claude-sonnet-4".to_string()),
            task_timeout: self.task_timeout,
            semaphore: Arc::new(Semaphore::new(self.max_concurrent_subagents)),
            handles: Arc::new(Mutex::new(HashMap::new())),
            agent_task_repo: self.agent_task_repo.expect("agent_task_repo is required"),
            tool_kit: std::sync::Mutex::new(self.tool_kit),
            hook_engine: std::sync::Mutex::new(self.hook_engine),
            registry_cache: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl SubagentManager {
    /// Create a new subagent manager builder
    pub fn builder(provider: DynProvider, workspace: PathBuf) -> SubagentManagerBuilder {
        SubagentManagerBuilder::new(provider, workspace)
    }

    /// Inject the shared tool-kit builder (called by app-core after AgentRuntime is built).
    pub fn set_tool_kit(&self, kit: Arc<klynt_core::ToolKitBuilder>) {
        let mut lock = self.tool_kit.lock().unwrap();
        *lock = Some(kit);
        // ToolKitBuilder deps changed → cached registries are stale.
        let mut cache = self.registry_cache.lock().unwrap();
        cache.clear();
    }

    pub fn set_hook_engine(&self, engine: Arc<klynt_hooks::HookEngine>) {
        let mut lock = self.hook_engine.lock().unwrap();
        *lock = Some(engine);
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
        let short_id = subagent_id[..8].to_string();
        let label_text = label.unwrap_or_else(|| "background task".to_string());
        let cancel_token = CancellationToken::new();

        debug!("Spawning subagent {} for task: {}", subagent_id, task);

        let tool_kit = {
            let lock = self.tool_kit.lock().unwrap();
            lock.clone()
        };
        if tool_kit.is_none() {
            return format!(
                "Error: subagent tool kit not initialized (ID: {})",
                short_id
            );
        }

        let provider = Arc::clone(&self.provider);
        let workspace = self.workspace.clone();
        let inbound_tx = self.inbound_tx.clone();
        let model = self.model.clone();
        let origin_key = format!("{}:{}", origin_channel, origin_chat_id);

        // Fire SubagentSpawn hook; abort if blocked.
        let hook_engine = {
            let lock = self.hook_engine.lock().unwrap();
            lock.clone()
        };
        if let Some(ref engine) = hook_engine {
            let input = klynt_hooks::events::subagent_spawn::SubagentSpawnInput {
                session_id: origin_key.clone(),
                parent_session_id: None,
                profile: profile.to_string(),
                task_summary: label_text.clone(),
                base: Default::default(),
            };
            match engine
                .fire(klynt_hooks::engine::HookFireInput::SubagentSpawn(input))
                .await
            {
                klynt_hooks::HookOutcome::Block { reason } => {
                    return format!(
                        "Subagent spawn blocked by hook: {} (ID: {})",
                        reason, short_id
                    );
                }
                _ => {}
            }
        }

        // Store handle for cancel/status tracking (only after hook allows)
        {
            let mut handles = self.handles.lock().await;
            handles.insert(
                short_id.clone(),
                SubagentHandle {
                    cancel_token: cancel_token.clone(),
                    label: label_text.clone(),
                    profile,
                    spawned_at: std::time::Instant::now(),
                },
            );
        }
        let subagent_id_clone = subagent_id.clone();
        let label_clone = label_text.clone();
        let config = SubagentConfig {
            task_timeout: self.task_timeout,
            agent_task_repo: self.agent_task_repo.clone(),
            session_key: origin_key.clone(),
            agent_id: short_id.clone(),
        };

        // Build or retrieve cached base registry (without AgentTaskTool).
        let base_registry = if let Some(ref kit) = tool_kit {
            let kit = (**kit).clone().with_cwd(workspace.clone());
            let cache_key = (profile, workspace.clone());
            let mut cache = self.registry_cache.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                Some(cached.clone())
            } else {
                let mut tools = ToolRegistry::new();
                match profile {
                    SubagentProfile::ReadOnly => {
                        kit.register_read_only(&mut tools);
                    }
                    SubagentProfile::ReadWrite => {
                        kit.register_read_only(&mut tools);
                        kit.register_mutating(&mut tools);
                    }
                    SubagentProfile::Full => {
                        kit.register_all(&mut tools);
                    }
                }
                let arc = Arc::new(RwLock::new(tools));
                cache.insert(cache_key, arc.clone());
                Some(arc)
            }
        } else {
            None
        };

        let semaphore = Arc::clone(&self.semaphore);
        let handles_ref = Arc::clone(&self.handles);
        let short_id_clone = short_id.clone();

        tokio::spawn(async move {
            // Acquire permit before running — limits concurrent subagents.
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("Semaphore closed unexpectedly");

            info!("Subagent {} started: {}", subagent_id_clone, label_clone);

            // Run task with cancellation support
            let result = tokio::select! {
                _ = cancel_token.cancelled() => {
                    Err("Cancelled by user".into())
                }
                r = run_subagent_task(&provider, &workspace, &model, &task, config, profile, tool_kit, hook_engine.clone(), origin_key.clone(), base_registry) => {
                    r
                }
            };

            // Remove handle after completion
            {
                let mut handles = handles_ref.lock().await;
                handles.remove(&short_id_clone);
            }

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
            short_id, label_text
        )
    }

    /// Cancel a running subagent by its short ID.
    pub async fn cancel_subagent(&self, agent_id: &str) -> common::Result<String> {
        let mut handles = self.handles.lock().await;
        if let Some(handle) = handles.remove(agent_id) {
            handle.cancel_token.cancel();
            Ok(format!(
                "Cancelled subagent '{}' ({})",
                handle.label, agent_id
            ))
        } else {
            Err(common::ToolError::ExecutionFailed(format!(
                "No running subagent with ID '{}'",
                agent_id
            ))
            .into())
        }
    }

    /// Get status of all running subagents.
    pub async fn get_status(&self) -> common::Result<String> {
        let handles = self.handles.lock().await;
        if handles.is_empty() {
            return Ok("No subagents currently running.".to_string());
        }

        let mut lines = vec!["Running subagents:".to_string()];
        for (id, handle) in handles.iter() {
            let elapsed = handle.spawned_at.elapsed();
            lines.push(format!(
                "  {} — '{}' [{}] (running for {}s)",
                id,
                handle.label,
                handle.profile,
                elapsed.as_secs()
            ));
        }
        Ok(lines.join("\n"))
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

    async fn cancel(&self, agent_id: &str) -> common::Result<String> {
        self.cancel_subagent(agent_id).await
    }

    async fn status(&self, _session_key: &str) -> common::Result<String> {
        self.get_status().await
    }
}

/// Run an agent loop for a subagent task with profile-based tools.
///
/// Tool access is determined by `SubagentProfile`:
/// - ReadOnly: read-only / network / interaction tools
/// - ReadWrite: read-only + mutating tools
/// - Full: all tools including plan-mode
async fn run_subagent_task(
    provider: &DynProvider,
    workspace: &std::path::Path,
    model: &str,
    task: &str,
    config: SubagentConfig,
    profile: SubagentProfile,
    tool_kit: Option<Arc<klynt_core::ToolKitBuilder>>,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    session_key: String,
    base_registry: Option<Arc<RwLock<ToolRegistry>>>,
) -> std::result::Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    use crate::execution::budget::{DepthMode, ExecutionBudget};
    use crate::execution::core::ExecutionCore;
    use crate::execution::execute_loop::execute_loop;
    use crate::execution::types::ExecutionParams;
    use tokio::sync::RwLock;

    let mut tools = if let Some(cached) = base_registry {
        // Clone the cached base registry and append the per-invocation AgentTaskTool.
        let mut t = (*cached.read().await).clone();
        let task_handler = Arc::new(crate::adapters::agent_task::AgentTaskHandlerImpl::new(
            config.agent_task_repo,
        ));
        t.register(AgentTaskTool::new(
            task_handler,
            config.session_key,
            config.agent_id,
        ));
        t
    } else {
        let kit = tool_kit.ok_or_else(|| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "subagent tool kit not initialized",
            )) as Box<dyn std::error::Error + Send + Sync>
        })?;
        let kit = (*kit).clone().with_cwd(workspace.to_path_buf());

        let mut tools = ToolRegistry::new();
        match profile {
            SubagentProfile::ReadOnly => {
                kit.register_read_only(&mut tools);
            }
            SubagentProfile::ReadWrite => {
                kit.register_read_only(&mut tools);
                kit.register_mutating(&mut tools);
            }
            SubagentProfile::Full => {
                kit.register_all(&mut tools);
            }
        }

        let task_handler = Arc::new(crate::adapters::agent_task::AgentTaskHandlerImpl::new(
            config.agent_task_repo,
        ));
        tools.register(AgentTaskTool::new(
            task_handler,
            config.session_key,
            config.agent_id,
        ));
        tools
    };

    let tool_defs = tools.get_definitions();

    // Build execution engine
    let core = Arc::new(ExecutionCore::new(
        Arc::clone(provider),
        Arc::new(RwLock::new(tools)),
    ));
    // Build system prompt and messages
    let system_prompt = build_subagent_prompt(workspace, task, profile);
    let messages = vec![
        Message::system(system_prompt),
        Message::user(task.to_string()),
    ];

    let params = ExecutionParams::new(model)
        .with_timeout(std::time::Duration::from_secs(config.task_timeout));
    let mut routing_ctx = RoutingContext::new("subagent".into(), "background".into());
    routing_ctx.hook_engine = hook_engine;
    routing_ctx.session_key = Some(session_key.into());

    // Execute via unified execute loop with a fixed budget
    let mut budget = ExecutionBudget::with_limits(
        DepthMode::Normal,
        120_000, // generous token budget for subagents
        profile.max_iterations(),
    );
    let result = execute_loop(
        &core,
        messages,
        &tool_defs,
        &params,
        &mut budget,
        &routing_ctx,
        None,
    )
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(("ok".to_string(), result.content))
}

/// Build a focused system prompt for subagents, tailored to the profile.
fn build_subagent_prompt(
    workspace: &std::path::Path,
    task: &str,
    profile: SubagentProfile,
) -> String {
    let tool_description = profile.tool_description();

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
- Manage tasks on the shared task board (agent_task tool: list, claim, complete, fail)

## What You Cannot Do
- Send messages directly to users (no message tool available)
- Spawn other subagents
- Access the main agent's conversation history

## Task Board
Use the `agent_task` tool to coordinate with other subagents:
- `list` — see all tasks for this session
- `claim` — take ownership of an unclaimed task
- `complete` — mark a task done with your results
- `fail` — report that a task failed

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

    async fn make_manager(permits: usize) -> SubagentManager {
        let (tx, _rx) = mpsc::channel(8);
        let provider: DynProvider = Arc::new(NoOpProvider);
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::AgentTaskRepo::new(pool.inner().clone());
        SubagentManager::builder(provider, std::path::PathBuf::from("/tmp"))
            .inbound_sender(tx)
            .max_concurrent_subagents(permits)
            .agent_task_repo(repo)
            .build()
    }

    fn make_tool_kit(pool: &storage::StoragePool) -> Arc<klynt_core::ToolKitBuilder> {
        let layer1 = Arc::new(
            klynt_core::approval::Layer1::compile(&config::schema::CodingPermissions::default())
                .unwrap(),
        );
        let policy = Arc::new(klynt_execpolicy::Policy::empty());
        let privacy = Arc::new(klynt_core::privacy::PrivacyGuard::from_globs(&[]).unwrap());
        let pending = Arc::new(klynt_core::approval::PendingApprovalsMap::default());
        let bus = Arc::new(bus::DomainEventBus::new(16));
        let repos = storage::Repos::from_pool(pool);
        let host_cache = Arc::new(klynt_core::approval::HostApprovalCache::default());
        let non_ui_policy = common::tool_channel::NonUiPolicy::Allow;

        Arc::new(klynt_core::ToolKitBuilder {
            cwd: std::path::PathBuf::from("/tmp"),
            layer1,
            policy,
            privacy,
            pending,
            bus,
            repos,
            host_cache,
            non_ui_policy,
            hook_engine: None,
            snapshot_repo: None,
            session_key: String::new(),
            history_repo: None,
            mirror_learning_enabled: false,
            mirror_min_approvals: 0,
            mirror_cooldown_seconds: 0,
            repo_id: String::new(),
        })
    }

    #[tokio::test]
    async fn test_subagent_manager_has_semaphore() {
        let mgr = make_manager(5).await;
        assert_eq!(mgr.semaphore_permits(), 5);
    }

    #[tokio::test]
    async fn test_run_subagent_task_returns_text_response() {
        let provider: DynProvider = Arc::new(NoOpProvider);
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::AgentTaskRepo::new(pool.inner().clone());
        let kit = make_tool_kit(&pool);
        let config = SubagentConfig {
            task_timeout: 60,
            agent_task_repo: repo,
            session_key: "test:session".to_string(),
            agent_id: "agent-1".to_string(),
        };
        let result = run_subagent_task(
            &provider,
            std::path::Path::new("/tmp"),
            "no-op",
            "Say hello",
            config,
            SubagentProfile::ReadWrite,
            Some(kit),
            None,
            "test:session".to_string(),
            None,
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
    fn test_subagent_profile_default_is_read_only() {
        let profile = SubagentProfile::default();
        assert!(matches!(profile, SubagentProfile::ReadOnly));
    }

    #[test]
    fn test_subagent_profile_from_str() {
        assert!(matches!(
            SubagentProfile::from_str("research"),
            Ok(SubagentProfile::ReadOnly)
        ));
        assert!(matches!(
            SubagentProfile::from_str("analyst"),
            Ok(SubagentProfile::ReadOnly)
        ));
        assert!(matches!(
            SubagentProfile::from_str("read_only"),
            Ok(SubagentProfile::ReadOnly)
        ));
        assert!(matches!(
            SubagentProfile::from_str("general"),
            Ok(SubagentProfile::ReadWrite)
        ));
        assert!(matches!(
            SubagentProfile::from_str("read_write"),
            Ok(SubagentProfile::ReadWrite)
        ));
        assert!(matches!(
            SubagentProfile::from_str("full"),
            Ok(SubagentProfile::Full)
        ));
        assert!(matches!(
            SubagentProfile::from_str("code"),
            Ok(SubagentProfile::Full)
        ));
        // Unknown profiles default to ReadWrite
        assert!(matches!(
            SubagentProfile::from_str("unknown"),
            Ok(SubagentProfile::ReadWrite)
        ));
    }

    #[test]
    fn test_subagent_profile_max_iterations() {
        assert_eq!(SubagentProfile::Full.max_iterations(), 15);
        assert_eq!(SubagentProfile::ReadWrite.max_iterations(), 10);
        assert_eq!(SubagentProfile::ReadOnly.max_iterations(), 5);
    }

    #[test]
    fn test_build_subagent_prompt_includes_profile_role() {
        let prompt = build_subagent_prompt(
            std::path::Path::new("/tmp"),
            "Research Rust patterns",
            SubagentProfile::ReadOnly,
        );
        assert!(prompt.contains("analyst"));
        assert!(prompt.contains("Research Rust patterns"));
        assert!(prompt.contains("read-only"));
    }

    #[test]
    fn test_build_subagent_prompt_read_write_profile() {
        let prompt = build_subagent_prompt(
            std::path::Path::new("/tmp"),
            "Do something",
            SubagentProfile::ReadWrite,
        );
        assert!(prompt.contains("research specialist"));
        assert!(prompt.contains("shell commands"));
    }

    #[test]
    fn test_build_subagent_prompt_full_profile() {
        let prompt = build_subagent_prompt(
            std::path::Path::new("/tmp"),
            "Analyze data",
            SubagentProfile::Full,
        );
        assert!(prompt.contains("general-purpose subagent"));
        assert!(prompt.contains("plan mode"));
    }
}
