//! Context builder for assembling system prompts.

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, warn};

use crate::providers::{ContentPart, ImageUrl, Message};
use crate::session::SessionMessage;

use super::{MemoryStore, SkillManager};

/// Context builder for agent prompts
pub struct ContextBuilder {
    workspace: PathBuf,
    memory: MemoryStore,
    skills: SkillManager,
    cached_bootstrap: Option<String>,
}

impl ContextBuilder {
    /// Create a new context builder
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            memory: MemoryStore::new(workspace.clone()),
            skills: SkillManager::new(),
            workspace,
            cached_bootstrap: None,
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
        let mut messages = Vec::new();

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
                let mut parts = Vec::new();
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

    /// Build system prompt from all sources
    async fn build_system_prompt(&mut self, channel: &str, chat_id: &str) -> String {
        let mut sections = Vec::new();

        // Identity section (always fresh - contains runtime info)
        sections.push(self.build_identity_section(channel, chat_id));

        // Bootstrap files (cached)
        if self.cached_bootstrap.is_none() {
            // First time: read and cache all bootstrap files
            let mut bootstrap_sections = Vec::new();

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

            self.cached_bootstrap = Some(bootstrap_sections.join("\n\n---\n\n"));
            debug!("Cached bootstrap files");
        }

        // Add cached bootstrap content
        if let Some(bootstrap) = &self.cached_bootstrap {
            sections.push(bootstrap.clone());
        }

        // Memory (always fresh)
        let memory_context = self.memory.get_memory_context().await;
        if !memory_context.trim().is_empty() {
            sections.push(format!("# Memory\n\n{}", memory_context));
        }

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
        let date_str = now.format("%Y-%m-%d %H:%M (%A)").to_string();

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

    /// Invalidate the bootstrap cache (call when bootstrap files change)
    pub fn invalidate_cache(&mut self) {
        self.cached_bootstrap = None;
        debug!("Invalidated bootstrap cache");
    }
}
