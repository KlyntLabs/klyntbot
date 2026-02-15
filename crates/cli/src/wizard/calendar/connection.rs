//! Apple Calendar configuration and CalDAV connection testing.

use anyhow::Result;
use common::utils::terminal::*;
use config::schema::Secret;
use config::Config;

use crate::wizard::prompts;

// ============================================================================
// Apple Calendar Configuration
// ============================================================================

/// Configure Apple Calendar credentials (Apple ID + app-specific password).
pub(super) async fn configure_apple_calendar(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    // Apple ID email
    let existing_username = &config.calendar.username;
    let username_default = if existing_username.is_empty() {
        None
    } else {
        Some(existing_username.as_str())
    };
    let username = super::prompt_text_with_validation(
        "Apple ID email",
        username_default,
        true,
        super::validate_email,
    )?;
    config.calendar.username = username;

    // App-specific password
    let existing_password = config.calendar.password.expose().clone();
    let password = if existing_password.is_empty() {
        display_password_instructions(chars);
        prompts::prompt_secret("App-specific password", 16)?
    } else {
        match prompts::prompt_secret_with_existing("App-specific password", &existing_password, 16)?
        {
            Some(new_password) => new_password,
            None => existing_password,
        }
    };
    config.calendar.password = Secret::new(password);

    // Calendar name
    let cal_default = if config.calendar.calendar_name.is_empty() {
        "Klyntbot"
    } else {
        &config.calendar.calendar_name
    };
    let calendar_name = prompts::prompt_text("Calendar name", Some(cal_default), false)?;
    config.calendar.calendar_name = calendar_name;

    // Sync interval
    let interval_options = vec![
        prompts::SelectOption {
            label: "1 minute",
            description: "Most frequent (higher battery usage)",
        },
        prompts::SelectOption {
            label: "5 minutes",
            description: "Recommended balance",
        },
        prompts::SelectOption {
            label: "15 minutes",
            description: "Less frequent updates",
        },
        prompts::SelectOption {
            label: "30 minutes",
            description: "Least frequent",
        },
    ];

    let interval_default = match config.calendar.sync_interval_secs {
        0..=60 => 0,
        61..=300 => 1,
        301..=900 => 2,
        _ => 3,
    };
    let selected = prompts::prompt_select("Sync interval", &interval_options, interval_default)?;

    config.calendar.sync_interval_secs = match selected {
        0 => 60,
        1 => 300,
        2 => 900,
        3 => 1800,
        _ => 300,
    };

    // Set defaults
    config.calendar.enabled = true;
    config.calendar.caldav_url = "https://caldav.icloud.com".to_string();

    Ok(())
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

// ============================================================================
// Connection Testing
// ============================================================================

/// Test a CalDAV connection by attempting to reach the server.
pub(super) async fn test_caldav_connection(
    url: &str,
    username: &str,
    password: &str,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client
        .request(reqwest::Method::from_bytes(b"PROPFIND")?, url)
        .header("Depth", "0")
        .header("Content-Type", "application/xml")
        .basic_auth(username, Some(password))
        .body(
            r#"<?xml version="1.0" encoding="utf-8"?>
<propfind xmlns="DAV:">
  <prop>
    <current-user-principal/>
  </prop>
</propfind>"#,
        )
        .send()
        .await?;

    if resp.status().is_success() || resp.status().as_u16() == 207 {
        Ok(())
    } else if resp.status().as_u16() == 401 {
        anyhow::bail!("Authentication failed — check credentials")
    } else {
        anyhow::bail!("HTTP {}", resp.status())
    }
}
