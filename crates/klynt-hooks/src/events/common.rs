use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct BaseEventInput {
    #[serde(default)]
    pub cwd: Option<String>,
}

pub fn matcher_pattern_for_event(
    event_name: HookEventName,
    matcher: Option<&str>,
) -> Option<String> {
    match event_name {
        HookEventName::PreToolUse | HookEventName::PostToolUse => {
            matcher.map(|m| format!("tool:{m}"))
        }
        HookEventName::PreFileEdit | HookEventName::PostFileEdit => {
            matcher.map(|m| format!("file:{m}"))
        }
        HookEventName::SubagentSpawn => matcher.map(|m| format!("profile:{m}")),
        _ => matcher.map(String::from),
    }
}

pub fn validate_matcher_pattern(_pattern: &str) -> Result<(), String> {
    Ok(())
}
