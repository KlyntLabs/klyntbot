//! Identity context source — runtime info (date, OS, workspace, channel).

use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use context_engine::source::{ContextSource, SourceContext};

/// Provides the identity section of the system prompt.
///
/// Always fresh (never cached) because it contains runtime info like
/// the current date/time.
pub struct IdentitySource {
    workspace: PathBuf,
    timezone: String,
    /// Pre-parsed timezone — avoids re-parsing on every `provide()` call.
    parsed_tz: Option<chrono_tz::Tz>,
}

impl IdentitySource {
    pub fn new(workspace: PathBuf, timezone: String) -> Self {
        let parsed_tz = timezone.parse::<chrono_tz::Tz>().ok();
        Self {
            workspace,
            timezone,
            parsed_tz,
        }
    }
}

#[async_trait]
impl ContextSource for IdentitySource {
    fn name(&self) -> &str {
        "identity"
    }

    fn priority(&self) -> u8 {
        100
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let now = Utc::now();

        let date_str = if let Some(tz) = self.parsed_tz {
            let local = now.with_timezone(&tz);
            let utc_offset = common::utils::date::timezone_utc_offset(&self.timezone);
            format!(
                "{} ({}, UTC{})",
                local.format("%Y-%m-%d %H:%M (%A)"),
                self.timezone,
                utc_offset
            )
        } else {
            now.format("%Y-%m-%d %H:%M (%A) (UTC)").to_string()
        };

        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        // In direct mode (CLI/dashboard), responses stream directly to the user
        // so the LLM should produce text responses, not call the `message` tool.
        // For bus-driven channels (Telegram, Discord, etc.), the LLM must use
        // the `message` tool to route responses to the correct channel/chat.
        let messaging_instruction = if ctx.channel == common::CLI_CHANNEL {
            "- Respond directly with text — do NOT use the `message` tool (your text response is streamed to the user automatically)\n\
             - Only use the `message` tool if you need to send to a different channel"
        } else {
            "- Use the `message` tool to send responses to the user\n\
             - Only use the `message` tool for actual communication — don't use it for internal reasoning"
        };

        Some(format!(
            r#"# Identity

You are klyntbot, a personal AI assistant powered by advanced language models.

**Current Context:**
- Date/Time: {}
- OS: {} ({})
- Workspace: {}
- Channel: {}
- Chat ID: {}

**Important Instructions:**
{}
- Use other tools (read_file, web_search, etc.) to gather information before responding
- Always be helpful, accurate, and concise

**Interactive Clarification:**
- Use the `ask_user` tool when you need clarification, preferences, or decisions from the user
- Group related questions (1-4) into a single ask_user call for better UX
- **CRITICAL:** Never call ask_user alongside other tools in the same turn - it blocks until the user responds
- ask_user supports: single-select, multi-select, yes/no, and free-text questions
- Prefer ask_user over conversational back-and-forth when you need structured choices

**Creating To-Do Tasks:** Follow the instructions in the `todo` skill (always loaded).

**Progressive Disclosure:**
- When skills reference additional documentation via markdown links, use read_file to load that documentation if the current task requires deeper knowledge
- Follow nested references recursively when needed for complex tasks
- Start with the summary in the skill, only load Deep Dive docs when the task demands it
"#,
            date_str,
            os,
            arch,
            self.workspace.display(),
            ctx.channel,
            ctx.chat_id,
            messaging_instruction
        ))
    }
}
