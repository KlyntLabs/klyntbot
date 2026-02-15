//! Generic CalDAV configuration wizard.
//!
//! Supports any CalDAV server with Basic auth: Nextcloud, Fastmail, Zoho,
//! Radicale, Baikal, etc.

use anyhow::Result;
use common::utils::terminal::*;
use config::schema::Secret;
use config::{CalendarProviderConfig, Config, GenericCalDavConfig};

use crate::wizard::prompts;

use super::connection::test_caldav_connection;

/// Configure a generic CalDAV provider.
pub(super) async fn configure_generic_caldav(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize("Generic CalDAV Setup", BOLD))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "Connect any CalDAV server (Nextcloud, Fastmail, Zoho, Radicale, etc.)",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Provider label
    let existing_name = config
        .calendar
        .providers
        .iter()
        .find_map(|p| match p {
            CalendarProviderConfig::GenericCalDav(c) => Some(c.name.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let name_default = if existing_name.is_empty() {
        None
    } else {
        Some(existing_name.as_str())
    };
    let provider_name = prompts::prompt_text(
        "Provider name (e.g., Nextcloud, Fastmail)",
        name_default,
        true,
    )?;

    // CalDAV URL
    let existing_url = config
        .calendar
        .providers
        .iter()
        .find_map(|p| match p {
            CalendarProviderConfig::GenericCalDav(c) => Some(c.caldav_url.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let url_default = if existing_url.is_empty() {
        None
    } else {
        Some(existing_url.as_str())
    };

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize("Common CalDAV URLs:", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  Nextcloud: https://cloud.example.com/remote.php/dav",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  Fastmail:  https://caldav.fastmail.com/dav/calendars/user/EMAIL/CALENDAR/",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  Radicale:  https://example.com/radicale/user/calendar.ics/",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    let caldav_url = prompts::prompt_text("CalDAV URL", url_default, true)?;

    // Username
    let existing_username = config
        .calendar
        .providers
        .iter()
        .find_map(|p| match p {
            CalendarProviderConfig::GenericCalDav(c) => Some(c.username.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let username_default = if existing_username.is_empty() {
        None
    } else {
        Some(existing_username.as_str())
    };
    let username = prompts::prompt_text("Username", username_default, true)?;

    // Password
    let existing_password = config
        .calendar
        .providers
        .iter()
        .find_map(|p| match p {
            CalendarProviderConfig::GenericCalDav(c) => Some(c.password.expose().clone()),
            _ => None,
        })
        .unwrap_or_default();
    let password = if existing_password.is_empty() {
        prompts::prompt_secret("Password", 0)?
    } else {
        match prompts::prompt_secret_with_existing("Password", &existing_password, 0)? {
            Some(new_password) => new_password,
            None => existing_password,
        }
    };

    // Calendar name
    let existing_cal = config
        .calendar
        .providers
        .iter()
        .find_map(|p| match p {
            CalendarProviderConfig::GenericCalDav(c) => Some(c.calendar_name.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let cal_default = if existing_cal.is_empty() {
        "Klyntbot"
    } else {
        &existing_cal
    };
    let calendar_name = prompts::prompt_text("Calendar name", Some(cal_default), false)?;

    // Test connection
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Testing connection...",
        colorize(chars.vertical, BRAND),
        colorize("●", BRAND)
    );
    match test_caldav_connection(&caldav_url, &username, &password).await {
        Ok(()) => {
            println!(
                "{} {} Connection successful!",
                colorize(chars.vertical, BRAND),
                status_success()
            );
        }
        Err(e) => {
            println!(
                "{} {} Connection failed: {}",
                colorize(chars.vertical, BRAND),
                status_error(),
                e
            );
            println!(
                "{}",
                draw_step_line(&colorize(
                    "Configuration saved anyway — you can test again later.",
                    DIM
                ))
            );
        }
    }

    // Remove existing generic CalDAV providers and add the new one
    config
        .calendar
        .providers
        .retain(|p| !matches!(p, CalendarProviderConfig::GenericCalDav(_)));

    config
        .calendar
        .providers
        .push(CalendarProviderConfig::GenericCalDav(GenericCalDavConfig {
            enabled: true,
            name: provider_name,
            caldav_url,
            username,
            password: Secret::new(password),
            calendar_name,
            sync_interval_secs: 300,
            auto_sync_due_dates: true,
        }));

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Generic CalDAV provider configured!",
        colorize(chars.vertical, BRAND),
        status_success()
    );

    Ok(())
}
