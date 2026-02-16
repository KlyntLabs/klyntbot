//! Todo notifications configuration wizard step.
//!
//! Configures where task reminders are sent (OS native, Telegram, Discord, etc.)
//! and notification preferences (daily digest, focus reminders).
//! On reconfiguration, shows current targets pre-checked and current values as defaults.

use anyhow::Result;

use crate::wizard::framework::{StepResult, WizardState};
use crate::wizard::prompts::{
    prompt_multi_select_with_defaults, prompt_text, prompt_yes_no, SelectOption,
};

/// Run the todo notifications configuration step.
///
/// No y/n gate - shows notification targets with currently configured ones
/// pre-checked, and all settings with current values as defaults.
pub fn run_todo_notification_step(state: &mut WizardState) -> Result<StepResult> {
    // Build notification target options and mapping
    let mut options = Vec::new();
    let mut target_names = Vec::new();

    // Always include OS Native
    options.push(SelectOption {
        label: "OS Native",
        description: "Desktop notifications (macOS/Linux/Windows)",
    });
    target_names.push("os_native");

    // Add configured channels as notification targets
    if state.config.channels.telegram.enabled {
        options.push(SelectOption {
            label: "Telegram",
            description: "Push to Telegram chat",
        });
        target_names.push("telegram");
    }
    if state.config.channels.discord.enabled {
        options.push(SelectOption {
            label: "Discord",
            description: "Push to Discord DM",
        });
        target_names.push("discord");
    }
    if state.config.channels.slack.enabled {
        options.push(SelectOption {
            label: "Slack",
            description: "Push to Slack DM",
        });
        target_names.push("slack");
    }
    if state.config.channels.email.enabled {
        options.push(SelectOption {
            label: "Email",
            description: "Send to configured email",
        });
        target_names.push("email");
    }

    // Build defaults from current config - pre-check currently configured targets
    let current_targets = &state.config.todo.notifications.targets;
    let defaults: Vec<bool> = target_names
        .iter()
        .map(|name| current_targets.iter().any(|t| t == name))
        .collect();

    // Prompt for notification targets with pre-checked defaults
    let selected_indices =
        prompt_multi_select_with_defaults("Notification targets", &options, &defaults)?;

    // Map selected indices to target names
    let targets: Vec<String> = selected_indices
        .iter()
        .filter_map(|&idx| target_names.get(idx).map(|s| s.to_string()))
        .collect();

    state.config.todo.notifications.targets = targets;

    // Configure focus reminders (default from current config)
    let enable_reminders = prompt_yes_no(
        "Enable focus deadline reminders?",
        state.config.todo.notifications.focus_reminders,
    )?;
    state.config.todo.notifications.focus_reminders = enable_reminders;

    // Configure daily digest (default from current config)
    let enable_digest = prompt_yes_no(
        "Enable daily task digest?",
        state.config.todo.notifications.daily_digest,
    )?;
    state.config.todo.notifications.daily_digest = enable_digest;

    if enable_digest {
        let current_time = &state.config.todo.notifications.daily_digest_time;
        let default_time = if current_time.is_empty() {
            "09:00"
        } else {
            current_time
        };
        let time = prompt_text("Daily digest time (HH:MM)", Some(default_time), false)?;
        state.config.todo.notifications.daily_digest_time = time;
    }

    // Configure daily planning (default from current config)
    let enable_planning = prompt_yes_no(
        "Enable daily planning (AI-powered morning task suggestions)?",
        state.config.todo.daily_planning.enabled,
    )?;
    state.config.todo.daily_planning.enabled = enable_planning;

    if enable_planning {
        let current_time = &state.config.todo.daily_planning.planning_time;
        let default_time = if current_time.is_empty() {
            "08:00"
        } else {
            current_time
        };
        let time = prompt_text("Daily planning time (HH:MM)", Some(default_time), false)?;
        state.config.todo.daily_planning.planning_time = time;
    }

    Ok(StepResult::Next)
}
