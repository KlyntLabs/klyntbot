//! Calendar sync configuration wizard step.
//!
//! Configures Apple Calendar sync via CalDAV including credentials,
//! calendar name, and sync interval.

use anyhow::Result;
use config::schema::Secret;

use crate::wizard::framework::{StepResult, WizardState};
use crate::wizard::prompts::{
    prompt_secret, prompt_select, prompt_text, prompt_yes_no, SelectOption,
};
use common::utils::terminal::*;

/// Run the calendar sync configuration step.
///
/// Prompts the user to:
/// - Enable/disable Apple Calendar sync
/// - Enter Apple ID (email)
/// - Enter app-specific password with instructions
/// - Choose calendar name
/// - Select sync interval
///
/// All settings are saved to `state.config.calendar`.
pub fn run_calendar_step(state: &mut WizardState) -> Result<StepResult> {
    let chars = BoxChars::get();

    // Step 1: Ask if user wants calendar sync
    if !prompt_yes_no("Enable Apple Calendar sync?", false)? {
        // User declined - skip configuration
        println!(
            "{}",
            draw_step_line(&colorize(
                "Calendar sync can be configured later with:",
                DIM
            ))
        );
        println!(
            "{}",
            draw_step_line(&colorize(
                "  klyntbot config set calendar.enabled true",
                DIM
            ))
        );
        println!("{}", colorize(chars.vertical, BRAND));
        return Ok(StepResult::Skip);
    }

    // Step 2: Prompt for Apple ID (email)
    let username = prompt_text_with_validation("Apple ID email", None, true, validate_email)?;
    state.config.calendar.username = username;

    // Step 3: Prompt for app-specific password with instructions
    display_password_instructions(chars);
    let password = prompt_secret("App-specific password", 16)?;
    state.config.calendar.password = Secret::new(password);

    // Step 4: Prompt for calendar name
    let calendar_name = prompt_text("Calendar name", Some("Klyntbot"), false)?;
    state.config.calendar.calendar_name = calendar_name;

    // Step 5: Select sync interval
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

    let selected = prompt_select("Sync interval", &interval_options, 1)?; // Default to 5 minutes

    let interval_secs = match selected {
        0 => 60,   // 1 minute
        1 => 300,  // 5 minutes
        2 => 900,  // 15 minutes
        3 => 1800, // 30 minutes
        _ => 300,  // Fallback to 5 minutes
    };

    state.config.calendar.sync_interval_secs = interval_secs;
    state.config.calendar.enabled = true;

    // Set iCloud CalDAV base URL for auto-discovery
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
