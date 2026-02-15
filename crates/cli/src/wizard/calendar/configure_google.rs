//! Google Calendar configuration wizard.
//!
//! Guides the user through OAuth2 setup for Google Calendar CalDAV access.
//! The user must create a GCP project and enable the CalDAV API first,
//! then we run an OAuth2 flow to get refresh tokens.

use anyhow::Result;
use common::utils::terminal::*;
use config::schema::Secret;
use config::Config;

use crate::wizard::oauth;
use crate::wizard::prompts;

/// Configure Google Calendar via OAuth2.
pub(super) async fn configure_google_calendar(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    // Instructions for GCP project setup
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize("Google Calendar Setup", BOLD))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "You'll need a Google Cloud project with the CalDAV API enabled.",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize("  1. Go to console.cloud.google.com", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  2. Create a project (or use an existing one)",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  3. Enable the 'Google Calendar API' in APIs & Services",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  4. Create OAuth 2.0 credentials (Desktop application type)",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  5. Add http://127.0.0.1 as an authorized redirect URI",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Client ID
    let client_id = oauth::prompt_client_id("Google Calendar")?;
    let google = config.calendar.ensure_google_mut();
    google.client_id = client_id.clone();

    // Client Secret
    let client_secret = oauth::prompt_client_secret("Google Calendar")?;
    let google = config.calendar.ensure_google_mut();
    google.client_secret = Secret::new(client_secret.clone());

    // Run OAuth flow
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize(
            "Starting OAuth2 authorization flow...",
            DIM
        ))
    );

    match oauth::run_google_oauth_flow(&client_id, &client_secret, 300).await? {
        Some(tokens) => {
            let google = config.calendar.ensure_google_mut();
            google.access_token = tokens.access_token;
            google.refresh_token = tokens
                .refresh_token
                .unwrap_or_else(|| Secret::new(String::new()));

            if google.refresh_token.expose().is_empty() {
                println!(
                    "{}{} Warning: No refresh token received. You may need to re-authorize later.",
                    colorize(chars.vertical, BRAND),
                    status_warning()
                );
                println!(
                    "{}",
                    draw_step_line(&colorize(
                        "Tip: Revoke access at myaccount.google.com/permissions and try again.",
                        DIM
                    ))
                );
            }
        }
        None => {
            println!(
                "{}{} OAuth flow was cancelled or timed out.",
                colorize(chars.vertical, BRAND),
                status_warning()
            );
            return Ok(());
        }
    }

    // Calendar ID
    let existing_cal_id = config
        .calendar
        .google()
        .map(|g| g.calendar_id.clone())
        .unwrap_or_else(|| "primary".to_string());
    let cal_default = if existing_cal_id.is_empty() {
        "primary"
    } else {
        &existing_cal_id
    };
    let calendar_id = prompts::prompt_text(
        "Calendar ID (\"primary\" for main calendar)",
        Some(cal_default),
        false,
    )?;
    let google = config.calendar.ensure_google_mut();
    google.calendar_id = if calendar_id.is_empty() {
        "primary".to_string()
    } else {
        calendar_id
    };

    // Calendar name for display
    let cal_name = config
        .calendar
        .google()
        .map(|g| g.calendar_name.clone())
        .unwrap_or_default();
    let name_default = if cal_name.is_empty() {
        "Klyntbot"
    } else {
        &cal_name
    };
    let calendar_name = prompts::prompt_text("Calendar name", Some(name_default), false)?;
    let google = config.calendar.ensure_google_mut();
    google.calendar_name = calendar_name;

    // Enable
    google.enabled = true;

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Google Calendar configured successfully!",
        colorize(chars.vertical, BRAND),
        status_success()
    );

    Ok(())
}
