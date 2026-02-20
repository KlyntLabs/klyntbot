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
}

impl IdentitySource {
    pub fn new(workspace: PathBuf, timezone: String) -> Self {
        Self {
            workspace,
            timezone,
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

        let date_str = if let Ok(tz) = self.timezone.parse::<chrono_tz::Tz>() {
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

**Creating To-Do Tasks:** Follow the instructions in the `todo` skill (always loaded).
"#,
            date_str,
            os,
            arch,
            self.workspace.display(),
            ctx.channel,
            ctx.chat_id
        ))
    }
}
