//! HookEngine — unified facade for firing all 13 hook events.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::error::{HookError, HookResult};
use crate::schema::{Hook, HookConfig};
use klynt_protocol::HookEventName;

pub mod command_runner;
pub mod discovery;
pub mod dispatcher;

pub mod schema_loader;

pub use dispatcher::{HookFireInput, HookOutcome};

#[derive(Debug, Clone, Default)]
pub struct HookEngine {
    handlers: HashMap<HookEventName, Vec<Hook>>,
}

impl HookEngine {
    pub fn empty() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn from_config(cfg: HookConfig) -> Self {
        let mut handlers: HashMap<HookEventName, Vec<Hook>> = HashMap::new();
        for hook in cfg.hook {
            handlers.entry(hook.event.clone()).or_default().push(hook);
        }
        Self { handlers }
    }

    pub fn load_from_path(path: &Path) -> HookResult<Self> {
        let s = std::fs::read_to_string(path)?;
        let cfg: HookConfig = toml::from_str(&s)?;
        Ok(Self::from_config(cfg))
    }

    pub async fn fire(&self, input: HookFireInput) -> HookOutcome {
        match input {
            HookFireInput::PreToolUse(i) => self.dispatch_pre_tool_use(i).await,
            HookFireInput::PostToolUse(i) => self.dispatch_post_tool_use(i).await,
            HookFireInput::SessionStart(i) => self.dispatch_session_start(i).await,
            HookFireInput::UserPromptSubmit(i) => self.dispatch_user_prompt_submit(i).await,
            HookFireInput::Stop(i) => self.dispatch_stop(i).await,
            HookFireInput::SessionEnd(i) => self.dispatch_session_end(i).await,
            HookFireInput::PreCompact(i) => self.dispatch_pre_compact(i).await,
            HookFireInput::PostCompact(i) => self.dispatch_post_compact(i).await,
            HookFireInput::PreFileEdit(i) => self.dispatch_pre_file_edit(i).await,
            HookFireInput::PostFileEdit(i) => self.dispatch_post_file_edit(i).await,
            HookFireInput::Notification(i) => self.dispatch_notification(i).await,
            HookFireInput::SubagentSpawn(i) => self.dispatch_subagent_spawn(i).await,
            HookFireInput::Error(i) => self.dispatch_error(i).await,
        }
    }

    async fn dispatch_pre_tool_use(
        &self,
        input: crate::events::pre_tool_use::PreToolUseInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::PreToolUse,
            &input,
            true,
            true,
        )
        .await
    }

    async fn dispatch_post_tool_use(
        &self,
        input: crate::events::post_tool_use::PostToolUseInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::PostToolUse,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_session_start(
        &self,
        input: crate::events::session_start::SessionStartInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::SessionStart,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_user_prompt_submit(
        &self,
        input: crate::events::user_prompt_submit::UserPromptSubmitInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::UserPromptSubmit,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_stop(&self, input: crate::events::stop::StopInput) -> HookOutcome {
        dispatcher::dispatch_event(&self.handlers, HookEventName::Stop, &input, false, false)
            .await
    }

    async fn dispatch_session_end(
        &self,
        input: crate::events::session_end::SessionEndInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::SessionEnd,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_pre_compact(
        &self,
        input: crate::events::pre_compact::PreCompactInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::PreCompact,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_post_compact(
        &self,
        input: crate::events::post_compact::PostCompactInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::PostCompact,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_pre_file_edit(
        &self,
        input: crate::events::pre_file_edit::PreFileEditInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::PreFileEdit,
            &input,
            true,
            true,
        )
        .await
    }

    async fn dispatch_post_file_edit(
        &self,
        input: crate::events::post_file_edit::PostFileEditInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::PostFileEdit,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_notification(
        &self,
        input: crate::events::notification::NotificationInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::Notification,
            &input,
            false,
            false,
        )
        .await
    }

    async fn dispatch_subagent_spawn(
        &self,
        input: crate::events::subagent_spawn::SubagentSpawnInput,
    ) -> HookOutcome {
        dispatcher::dispatch_event(
            &self.handlers,
            HookEventName::SubagentSpawn,
            &input,
            true,
            false,
        )
        .await
    }

    async fn dispatch_error(&self, input: crate::events::error::ErrorInput) -> HookOutcome {
        dispatcher::dispatch_event(&self.handlers, HookEventName::Error, &input, false, false)
            .await
    }
}
