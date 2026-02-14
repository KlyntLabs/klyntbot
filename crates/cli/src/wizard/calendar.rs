//! Calendar sync configuration wizard step.
//!
//! Configures Apple Calendar sync via CalDAV including credentials,
//! calendar name, and sync interval. On reconfiguration, shows current
//! values as defaults — Enter to keep.

use anyhow::Result;
use config::schema::Secret;

use crate::wizard::framework::{StepResult, WizardState};
use crate::wizard::prompts::{
    prompt_secret, prompt_secret_with_existing, prompt_select, prompt_text, SelectOption,
};
use common::utils::terminal::*;

/// Run the calendar sync configuration step.
///
/// Shows enabled/disabled as a select (no y/n gate). When enabled,
/// all fields show current values as defaults — Enter to keep.
pub fn run_calendar_step(state: &mut WizardState) -> Result<StepResult> {
    let chars = BoxChars::get();

    // Show current status
    if state.config.calendar.enabled {
        println!(
            "{} {} Calendar sync: {} ({})",
            colorize(chars.vertical, BRAND),
            colorize("✓", SUCCESS),
            colorize("enabled", SUCCESS),
            colorize(&state.config.calendar.username, DIM)
        );
        println!("{}", colorize(chars.vertical, BRAND));
    }

    println!(
        "{}",
        draw_step_line(&colorize(
            "Apple Calendar (CalDAV) sync with your todos.",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Step 1: Enabled/disabled select
    let enable_options = vec![
        SelectOption {
            label: "Enabled",
            description: "Sync todos to Apple Calendar",
        },
        SelectOption {
            label: "Disabled",
            description: "Skip calendar sync",
        },
    ];

    let enable_default = if state.config.calendar.enabled { 0 } else { 1 };
    let enable_idx = prompt_select("Calendar sync", &enable_options, enable_default)?;

    if enable_idx == 1 {
        // Disabled
        state.config.calendar.enabled = false;
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{}",
            draw_step_line(&colorize("Calendar sync can be enabled later with:", DIM))
        );
        println!(
            "{}",
            draw_step_line(&colorize(
                "  klyntbot config set calendar.enabled true",
                DIM
            ))
        );
        println!("{}", colorize(chars.vertical, BRAND));
        return Ok(StepResult::Next);
    }

    // Step 2: Apple ID email - edit-in-place
    let existing_username = &state.config.calendar.username;
    let username_default = if existing_username.is_empty() {
        None
    } else {
        Some(existing_username.as_str())
    };
    let username =
        prompt_text_with_validation("Apple ID email", username_default, true, validate_email)?;
    state.config.calendar.username = username;

    // Step 3: App-specific password - edit-in-place
    let existing_password = state.config.calendar.password.expose().clone();
    let password = if existing_password.is_empty() {
        display_password_instructions(chars);
        prompt_secret("App-specific password", 16)?
    } else {
        // Has existing password - allow Enter to keep
        match prompt_secret_with_existing("App-specific password", &existing_password, 16)? {
            Some(new_password) => new_password,
            None => existing_password,
        }
    };
    state.config.calendar.password = Secret::new(password);

    // Step 4: Calendar name - edit-in-place
    let cal_default = if state.config.calendar.calendar_name.is_empty() {
        "Klyntbot"
    } else {
        &state.config.calendar.calendar_name
    };
    let calendar_name = prompt_text("Calendar name", Some(cal_default), false)?;
    state.config.calendar.calendar_name = calendar_name;

    // Step 5: Sync interval - pre-select current
    let interval_options = vec![
        SelectOption {
            label: "1 minute",
            description: "Most frequent (higher battery usage)",
        },
        SelectOption {
            label: "5 minutes",
            description: "Recommended balance",
        },
        SelectOption {
            label: "15 minutes",
            description: "Less frequent updates",
        },
        SelectOption {
            label: "30 minutes",
            description: "Least frequent",
        },
    ];

    let interval_default = match state.config.calendar.sync_interval_secs {
        0..=60 => 0,
        61..=300 => 1,
        301..=900 => 2,
        _ => 3,
    };
    let selected = prompt_select("Sync interval", &interval_options, interval_default)?;

    let interval_secs = match selected {
        0 => 60,
        1 => 300,
        2 => 900,
        3 => 1800,
        _ => 300,
    };

    state.config.calendar.sync_interval_secs = interval_secs;
    state.config.calendar.enabled = true;
    state.config.calendar.caldav_url = "https://caldav.icloud.com".to_string();

    println!("{}", colorize(chars.vertical, BRAND));

    Ok(StepResult::Next)
}

/// Display instructions for generating an app-specific password.
fn display_password_instructions(chars: &BoxChars) {
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize("Generate an app-specific password:", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize("  1. Visit appleid.apple.com", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize("  2. Sign in with your Apple ID", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  3. Go to Security → App-Specific Passwords",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize("  4. Generate a password for \"Klyntbot\"", DIM))
    );
    println!("{}", colorize(chars.vertical, BRAND));
}

/// Prompt for text input with validation.
fn prompt_text_with_validation<F>(
    label: &str,
    default: Option<&str>,
    required: bool,
    validator: F,
) -> Result<String>
where
    F: Fn(&str) -> Result<(), String>,
{
    let prefix = {
        let chars = BoxChars::get();
        format!("{} ", colorize(chars.vertical, BRAND))
    };

    loop {
        let input = prompt_text(label, default, required)?;

        if input.is_empty() && !required {
            return Ok(input);
        }

        match validator(&input) {
            Ok(()) => return Ok(input),
            Err(msg) => {
                println!("{}{}", prefix, colorize(&msg, ERROR));
                continue;
            }
        }
    }
}

/// Validate email format (basic check).
fn validate_email(email: &str) -> Result<(), String> {
    if email.is_empty() {
        return Err("Email cannot be empty".to_string());
    }

    if !email.contains('@') {
        return Err("Invalid email format (missing @)".to_string());
    }

    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err("Invalid email format".to_string());
    }

    if !parts[1].contains('.') {
        return Err("Invalid email domain".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("test.user@domain.co.uk").is_ok());
        assert!(validate_email("user+tag@example.com").is_ok());
    }

    #[test]
    fn test_validate_email_invalid() {
        assert!(validate_email("").is_err());
        assert!(validate_email("notanemail").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("user@").is_err());
        assert!(validate_email("user@domain").is_err());
    }
}
