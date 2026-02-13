//! Todo notifications configuration wizard step.
//!
//! Configures where task reminders are sent (OS native, Telegram, Discord, etc.)
//! and notification preferences (daily digest, focus reminders).

use anyhow::Result;

use crate::wizard::framework::{StepResult, WizardState};
use crate::wizard::prompts::{prompt_multi_select, prompt_text, prompt_yes_no, SelectOption};

/// Run the todo notifications configuration step.
///
/// Prompts the user to:
/// - Enable/disable todo notifications
/// - Select notification targets (OS native + configured channels)
/// - Configure daily digest settings
/// - Configure focus reminder settings
///
/// All settings are saved to `state.config.todo.notifications`.
pub fn run_todo_notification_step(state: &mut WizardState) -> Result<StepResult> {
    // Ask if user wants todo notifications
    if !prompt_yes_no("Enable todo notifications?", true)? {
        // User declined - skip configuration
        return Ok(StepResult::Next);
    }

    // Build notification target options and mapping in parallel
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
    #[cfg(feature = "email")]
    if state.config.channels.email.enabled {
        options.push(SelectOption {
            label: "Email",
            description: "Send to configured email",
        });
        target_names.push("email");
    }

    // Prompt for notification targets
    let selected_indices = prompt_multi_select("Select notification targets", &options)?;

    // Map selected indices to target names
    let targets: Vec<String> = selected_indices
        .iter()
        .filter_map(|&idx| target_names.get(idx).map(|s| s.to_string()))
        .collect();

    state.config.todo.notifications.targets = targets;

    // Configure focus reminders
    let enable_reminders = prompt_yes_no("Enable focus deadline reminders?", true)?;
    state.config.todo.notifications.focus_reminders = enable_reminders;

    // Configure daily digest
    let enable_digest = prompt_yes_no("Enable daily task digest?", true)?;
    state.config.todo.notifications.daily_digest = enable_digest;

    if enable_digest {
        let time = prompt_text("Daily digest time (HH:MM)", Some("09:00"), false)?;
        state.config.todo.notifications.daily_digest_time = time;
    }

    Ok(StepResult::Next)
}
