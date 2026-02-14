//! Context builder for assembling system prompts.

use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, warn};

use providers::{ContentPart, ImageUrl, Message};
use session::SessionMessage;

use super::{MemoryStore, SkillManager};

/// Default TTL for cached contexts (seconds)
const CONTEXT_CACHE_TTL_SECS: i64 = 60;

/// Cached context string with TTL expiration
struct CachedContext {
    content: String,
    expires_at: DateTime<Utc>,
}

impl CachedContext {
    fn new(content: String, ttl_secs: i64) -> Self {
        Self {
            content,
            expires_at: Utc::now() + Duration::seconds(ttl_secs),
        }
    }

    fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }
}

/// Context builder for agent prompts
pub struct ContextBuilder {
    workspace: PathBuf,
    timezone: String,
    memory: MemoryStore,
    skills: SkillManager,
    cached_bootstrap: Option<String>,
    cached_memory: Option<CachedContext>,
    cached_todo: Option<CachedContext>,
    todo_store: Option<std::sync::Arc<tokio::sync::RwLock<tools::todo_store::TodoStore>>>,
}

impl ContextBuilder {
    /// Create a new context builder
    pub async fn new(
        workspace: PathBuf,
        timezone: String,
        todo_store: Option<std::sync::Arc<tokio::sync::RwLock<tools::todo_store::TodoStore>>>,
    ) -> Self {
        Self {
            memory: MemoryStore::new(workspace.clone()).await,
            skills: SkillManager::new(),
            workspace,
            timezone,
            cached_bootstrap: None,
            cached_memory: None,
            cached_todo: None,
            todo_store,
        }
    }

    /// Initialize skills
    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.skills.load(self.workspace.clone()).await?;
        Ok(())
    }

    /// Build messages array for LLM
    pub async fn build_messages(
        &mut self,
        history: Vec<SessionMessage>,
        current_message: &str,
        media: Option<Vec<String>>,
        channel: &str,
        chat_id: &str,
    ) -> Vec<Message> {
        // 1 system + up to 50 history + 1 current message
        let history_count = history.len().min(50);
        let mut messages = Vec::with_capacity(2 + history_count);

        // System prompt
        let system_prompt = self.build_system_prompt(channel, chat_id).await;
        messages.push(Message::system(system_prompt));

        // Add history (max 50 messages to keep context manageable)
        let history_start = history.len().saturating_sub(50);
        for msg in &history[history_start..] {
            match msg.role.as_str() {
                "user" => messages.push(Message::user(&msg.content)),
                "assistant" => messages.push(Message::assistant(&msg.content)),
                "system" => messages.push(Message::system(&msg.content)),
                _ => {}
            }
        }

        // Add current message
        if let Some(media_paths) = media {
            if !media_paths.is_empty() {
                // Multipart message with images
                let mut parts = Vec::with_capacity(1 + media_paths.len());
                parts.push(ContentPart::Text {
                    text: current_message.to_string(),
                });

                for path in media_paths {
                    if let Ok(data) = fs::read(&path).await {
                        let base64 = general_purpose::STANDARD.encode(&data);
                        let mime = mime_guess::from_path(&path)
                            .first_or_octet_stream()
                            .to_string();
                        let data_url = format!("data:{};base64,{}", mime, base64);

                        parts.push(ContentPart::ImageUrl {
                            image_url: ImageUrl { url: data_url },
                        });
                    }
                }

                messages.push(Message::user_multipart(parts));
            } else {
                messages.push(Message::user(current_message));
            }
        } else {
            messages.push(Message::user(current_message));
        }

        messages
    }

    /// Get todo context string from the store (with TTL cache)
    async fn get_todo_context(&mut self) -> String {
        // Return cached if still valid
        if let Some(ref cached) = self.cached_todo {
            if cached.is_valid() {
                return cached.content.clone();
            }
        }

        let content = if let Some(store) = &self.todo_store {
            let mut guard = store.write().await;
            guard.to_context_string().await.unwrap_or_default()
        } else {
            String::new()
        };

        self.cached_todo = Some(CachedContext::new(content.clone(), CONTEXT_CACHE_TTL_SECS));
        content
    }

    /// Get memory context (with TTL cache)
    async fn get_memory_context_cached(&mut self) -> String {
        // Return cached if still valid
        if let Some(ref cached) = self.cached_memory {
            if cached.is_valid() {
                return cached.content.clone();
            }
        }

        let content = self.memory.get_memory_context().await;
        self.cached_memory = Some(CachedContext::new(content.clone(), CONTEXT_CACHE_TTL_SECS));
        content
    }

    /// Build system prompt from all sources
    async fn build_system_prompt(&mut self, channel: &str, chat_id: &str) -> String {
        // identity + bootstrap + memory + todo + confidence + skills + always-loaded skills
        let mut sections = Vec::with_capacity(8);

        // Identity section (always fresh - contains runtime info)
        sections.push(self.build_identity_section(channel, chat_id));

        // Bootstrap files (cached)
        if self.cached_bootstrap.is_none() {
            // First time: read and cache all bootstrap files
            let mut bootstrap_sections = Vec::with_capacity(6);

            if let Some(agents) = self.read_bootstrap_file("AGENTS.md").await {
                bootstrap_sections.push(agents);
            }

            if let Some(soul) = self.read_bootstrap_file("SOUL.md").await {
                bootstrap_sections.push(soul);
            }

            if let Some(user) = self.read_bootstrap_file("USER.md").await {
                bootstrap_sections.push(user);
            }

            if let Some(tools) = self.read_bootstrap_file("TOOLS.md").await {
                bootstrap_sections.push(tools);
            }

            if let Some(identity) = self.read_bootstrap_file("IDENTITY.md").await {
                bootstrap_sections.push(identity);
            }

            if let Some(response) = self.read_bootstrap_file("RESPONSE.md").await {
                bootstrap_sections.push(response);
            }

            self.cached_bootstrap = Some(bootstrap_sections.join("\n\n---\n\n"));
            debug!("Cached bootstrap files");
        }

        // Add cached bootstrap content
        if let Some(bootstrap) = &self.cached_bootstrap {
            sections.push(bootstrap.clone());
        }

        // Memory (cached with TTL)
        let memory_context = self.get_memory_context_cached().await;
        if !memory_context.trim().is_empty() {
            sections.push(format!("# Memory\n\n{}", memory_context));
        }

        // Active tasks (cached with TTL)
        let todo_context = self.get_todo_context().await;
        if !todo_context.trim().is_empty() {
            sections.push(todo_context);
        }

        // Confidence evaluation instructions (internal)
        sections.push(super::confidence::prompt::CONFIDENCE_PROMPT.to_string());

        // Skills (relatively stable)
        let skills_summary = self.skills.generate_summary();
        sections.push(format!("# Available Skills\n\n{}", skills_summary));

        // Always-loaded skills (full content)
        for skill in self.skills.get_always_loaded() {
            if let Some(content) = &skill.content {
                sections.push(format!("# Skill: {}\n\n{}", skill.name, content));
            }
        }

        sections.join("\n\n---\n\n")
    }

    /// Build identity section with runtime info
    fn build_identity_section(&self, channel: &str, chat_id: &str) -> String {
        let now = Utc::now();

        // Format time in user's local timezone
        let date_str = if let Ok(tz) = self.timezone.parse::<chrono_tz::Tz>() {
            let local = now.with_timezone(&tz);
            format!(
                "{} ({})",
                local.format("%Y-%m-%d %H:%M (%A)"),
                self.timezone
            )
        } else {
            now.format("%Y-%m-%d %H:%M (%A) (UTC)").to_string()
        };

        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        format!(
            r#"# Identity

You are klyntbot, a personal AI assistant powered by advanced language models.

**Current Context:**
- Date/Time: {}
- OS: {} ({})
- Workspace: {}
- Channel: {}
- Chat ID: {}

**Important Instructions:**
- Use the `message` tool to send responses to the user
- Only use the `message` tool for actual communication - don't use it for internal reasoning
- Use other tools (read_file, web_search, etc.) to gather information before responding
- Always be helpful, accurate, and concise

**Interactive Clarification:**
- Use the `ask_user` tool when you need clarification, preferences, or decisions from the user
- Group related questions (1-4) into a single ask_user call for better UX
- **CRITICAL:** Never call ask_user alongside other tools in the same turn - it blocks until the user responds
- ask_user supports: single-select, multi-select, yes/no, and free-text questions
- Prefer ask_user over conversational back-and-forth when you need structured choices

**Creating To-Do Tasks:**
- **IMPORTANT:** When the user asks to create a todo task, use ask_user FIRST to gather details (title, description, priority, due date, tags)
- Do NOT create the task and then ask for improvements - get the information BEFORE creation
- After ask_user returns with answers, THEN call the todo tool with complete information
- This creates better tasks and avoids the need for updates
"#,
            date_str,
            os,
            arch,
            self.workspace.display(),
            channel,
            chat_id
        )
    }

    /// Read a bootstrap file from workspace
    async fn read_bootstrap_file(&self, filename: &str) -> Option<String> {
        let path = self.workspace.join(filename);

        if path.exists() {
            match fs::read_to_string(&path).await {
                Ok(content) => {
                    if !content.trim().is_empty() {
                        debug!("Loaded bootstrap file: {}", filename);
                        Some(content)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    warn!("Failed to read {}: {}", filename, e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Get skills manager reference
    pub fn skills(&self) -> &SkillManager {
        &self.skills
    }

    /// Get memory store reference
    pub fn memory(&self) -> &MemoryStore {
        &self.memory
    }

    /// Invalidate all caches (bootstrap, memory, todo)
    pub fn invalidate_cache(&mut self) {
        self.cached_bootstrap = None;
        self.cached_memory = None;
        self.cached_todo = None;
        debug!("Invalidated all context caches");
    }

    /// Invalidate only the memory context cache (call after memory writes)
    pub fn invalidate_memory_cache(&mut self) {
        self.cached_memory = None;
        debug!("Invalidated memory context cache");
    }

    /// Invalidate only the todo context cache (call after todo mutations)
    pub fn invalidate_todo_cache(&mut self) {
        self.cached_todo = None;
        debug!("Invalidated todo context cache");
    }
}
