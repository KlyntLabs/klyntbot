//! Calendar sync configuration wizard step.
//!
//! Provides an expand-in-place interactive menu for calendar provider
//! management, modeled after the channels step. Each calendar provider
//! (Apple Calendar, Google Calendar, Generic CalDAV) is an expandable row with
//! sub-actions for credentials, settings, and connection testing.

mod configure_generic;
mod configure_google;
mod connection;
mod menus;

use std::io::{self, IsTerminal};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

use super::prompts;

use configure_generic::configure_generic_caldav;
use configure_google::configure_google_calendar;
use connection::{configure_apple_calendar, test_caldav_connection};
use menus::{run_calendar_menu, run_calendar_menu_fallback};

// ============================================================================
// Calendar Provider Metadata
// ============================================================================

/// Metadata for a calendar provider in the selection UI.
pub(super) struct CalendarProviderInfo {
    pub(super) name: &'static str,
    pub(super) key: &'static str,
    pub(super) description: &'static str,
}

pub(super) const CALENDAR_PROVIDERS: &[CalendarProviderInfo] = &[
    CalendarProviderInfo {
        name: "Apple Calendar",
        key: "apple",
        description: "CalDAV sync with iCloud",
    },
    CalendarProviderInfo {
        name: "Google Calendar",
        key: "google",
        description: "OAuth2 sync with Google",
    },
    CalendarProviderInfo {
        name: "Generic CalDAV",
        key: "generic_caldav",
        description: "Nextcloud, Fastmail, Zoho, etc.",
    },
];

// ============================================================================
// Configuration Detection Helpers
// ============================================================================

/// Check if a calendar provider is configured (has credentials).
pub(super) fn is_provider_configured(config: &Config, provider_key: &str) -> bool {
    match provider_key {
        "apple" => config
            .calendar
            .apple()
            .is_some_and(|a| !a.username.is_empty() && !a.password.expose().is_empty()),
        "google" => config
            .calendar
            .google()
            .is_some_and(|g| !g.client_id.is_empty() && !g.refresh_token.expose().is_empty()),
        "generic_caldav" => {
            // Check if any generic CalDAV provider exists
            config.calendar.providers.iter().any(|p| {
                matches!(p, config::CalendarProviderConfig::GenericCalDav(c) if !c.caldav_url.is_empty() && !c.username.is_empty())
            })
        }
        _ => false,
    }
}

/// Check if a calendar provider is enabled.
pub(super) fn is_provider_enabled(config: &Config, provider_key: &str) -> bool {
    match provider_key {
        "apple" => config.calendar.apple().is_some_and(|a| a.enabled),
        "google" => config.calendar.google().is_some_and(|g| g.enabled),
        "generic_caldav" => config
            .calendar
            .providers
            .iter()
            .any(|p| matches!(p, config::CalendarProviderConfig::GenericCalDav(c) if c.enabled)),
        _ => false,
    }
}

/// Get a masked version of provider credentials for display.
fn mask_provider_credentials(config: &Config, provider_key: &str) -> String {
    match provider_key {
        "apple" => config
            .calendar
            .apple()
            .map(|a| a.username.clone())
            .unwrap_or_default(),
        "google" => config
            .calendar
            .google()
            .map(|g| {
                if g.calendar_id.is_empty() {
                    "configured".to_string()
                } else {
                    g.calendar_id.clone()
                }
            })
            .unwrap_or_default(),
        "generic_caldav" => config
            .calendar
            .providers
            .iter()
            .find_map(|p| match p {
                config::CalendarProviderConfig::GenericCalDav(c) => Some(c.name.clone()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Get a human-readable status description for a configured provider.
pub(super) fn get_provider_status_description(config: &Config, provider_key: &str) -> String {
    match provider_key {
        "apple" => {
            if let Some(apple) = config.calendar.apple() {
                if !apple.calendar_name.is_empty() {
                    let interval = format_interval(apple.sync_interval_secs);
                    format!("{} / {}", apple.calendar_name, interval)
                } else {
                    "configured".to_string()
                }
            } else {
                String::new()
            }
        }
        "google" => {
            if let Some(google) = config.calendar.google() {
                let cal = if google.calendar_id.is_empty() {
                    "primary"
                } else {
                    &google.calendar_id
                };
                let interval = format_interval(google.sync_interval_secs);
                format!("{} / {}", cal, interval)
            } else {
                String::new()
            }
        }
        "generic_caldav" => config
            .calendar
            .providers
            .iter()
            .find_map(|p| match p {
                config::CalendarProviderConfig::GenericCalDav(c) => {
                    let interval = format_interval(c.sync_interval_secs);
                    Some(format!("{} / {}", c.name, interval))
                }
                _ => None,
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Format sync interval seconds as a human-readable string.
pub(super) fn format_interval(secs: u64) -> String {
    match secs {
        0..=60 => "1 min".to_string(),
        61..=300 => "5 min".to_string(),
        301..=900 => "15 min".to_string(),
        _ => "30 min".to_string(),
    }
}

// ============================================================================
// State Model and Sub-Actions
// ============================================================================

/// Actions available in the expanded calendar provider sub-menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum CalendarSubAction {
    ConfigureCredentials,
    TestConnection,
    ToggleEnabled,
    ChangeCalendarName,
    ChangeSyncInterval,
    Close,
}

pub(super) const SUB_ACTIONS: &[CalendarSubAction] = &[
    CalendarSubAction::ConfigureCredentials,
    CalendarSubAction::TestConnection,
    CalendarSubAction::ToggleEnabled,
    CalendarSubAction::ChangeCalendarName,
    CalendarSubAction::ChangeSyncInterval,
    CalendarSubAction::Close,
];

impl CalendarSubAction {
    /// Check if this action is available given the provider's configuration state.
    pub(super) fn is_available(&self, configured: bool, _enabled: bool) -> bool {
        match self {
            Self::ConfigureCredentials | Self::Close => true,
            Self::TestConnection | Self::ToggleEnabled => configured,
            Self::ChangeCalendarName | Self::ChangeSyncInterval => configured,
        }
    }

    /// Get the display label for this action, adapting to the provider's state.
    pub(super) fn label(&self, config: &Config, provider: &CalendarProviderInfo) -> String {
        let configured = is_provider_configured(config, provider.key);
        let enabled = is_provider_enabled(config, provider.key);

        match self {
            Self::ConfigureCredentials => {
                if configured {
                    let masked = mask_provider_credentials(config, provider.key);
                    format!("Edit credentials ({})", masked)
                } else {
                    "Configure credentials".to_string()
                }
            }
            Self::TestConnection => {
                if configured {
                    "Test connection".to_string()
                } else {
                    "Test connection (requires credentials)".to_string()
                }
            }
            Self::ToggleEnabled => {
                let status = if enabled { "✓ enabled" } else { "disabled" };
                format!("Toggle enable/disable [{}]", status)
            }
            Self::ChangeCalendarName => {
                let name = get_calendar_name_for_provider(config, provider.key);
                if name.is_empty() {
                    "Calendar name".to_string()
                } else {
                    format!("Calendar name [{}]", name)
                }
            }
            Self::ChangeSyncInterval => {
                let secs = get_sync_interval_for_provider(config, provider.key);
                let interval = format_interval(secs);
                format!("Sync interval [{}]", interval)
            }
            Self::Close => "Close".to_string(),
        }
    }
}

/// Get the calendar name for a provider.
fn get_calendar_name_for_provider(config: &Config, provider_key: &str) -> String {
    match provider_key {
        "apple" => config
            .calendar
            .apple()
            .map(|a| a.calendar_name.clone())
            .unwrap_or_default(),
        "google" => config
            .calendar
            .google()
            .map(|g| g.calendar_name.clone())
            .unwrap_or_default(),
        "generic_caldav" => config
            .calendar
            .providers
            .iter()
            .find_map(|p| match p {
                config::CalendarProviderConfig::GenericCalDav(c) => Some(c.calendar_name.clone()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Get the sync interval for a provider.
fn get_sync_interval_for_provider(config: &Config, provider_key: &str) -> u64 {
    match provider_key {
        "apple" => config
            .calendar
            .apple()
            .map(|a| a.sync_interval_secs)
            .unwrap_or(300),
        "google" => config
            .calendar
            .google()
            .map(|g| g.sync_interval_secs)
            .unwrap_or(300),
        "generic_caldav" => config
            .calendar
            .providers
            .iter()
            .find_map(|p| match p {
                config::CalendarProviderConfig::GenericCalDav(c) => Some(c.sync_interval_secs),
                _ => None,
            })
            .unwrap_or(300),
        _ => 300,
    }
}

/// State for the interactive calendar menu.
pub(super) struct CalendarMenuState {
    pub(super) cursor: usize,
    pub(super) expanded: Option<usize>,
    pub(super) sub_cursor: usize,
    pub(super) in_sub_menu: bool,
}

impl CalendarMenuState {
    pub(super) fn new() -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
        }
    }

    pub(super) fn total_main_items(&self) -> usize {
        CALENDAR_PROVIDERS.len() + 1 // providers + "Done"
    }

    pub(super) fn is_on_done(&self) -> bool {
        self.cursor == CALENDAR_PROVIDERS.len()
    }
}

// ============================================================================
// Sub-Action Executors
// ============================================================================

/// Execute the configure credentials action for a calendar provider.
#[allow(clippy::single_match)]
pub(super) async fn execute_configure_credentials(
    config: &mut Config,
    provider_idx: usize,
) -> Result<()> {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

    match provider.key {
        "apple" => configure_apple_calendar(config).await?,
        "google" => configure_google_calendar(config).await?,
        "generic_caldav" => configure_generic_caldav(config).await?,
        _ => {}
    }

    Ok(())
}

/// Execute the test connection action for a calendar provider.
#[allow(clippy::single_match)]
pub(super) async fn execute_test_connection(config: &Config, provider_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let provider = &CALENDAR_PROVIDERS[provider_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{} Testing {} connection...",
        prefix,
        colorize("●", BRAND),
        provider.name
    );

    match provider.key {
        "apple" => {
            if let Some(apple) = config.calendar.apple() {
                let result = test_caldav_connection(
                    &apple.caldav_url,
                    &apple.username,
                    apple.password.expose(),
                )
                .await;

                match result {
                    Ok(()) => {
                        println!(
                            "{}{} Connection successful — {}",
                            prefix,
                            status_success(),
                            apple.username
                        );
                    }
                    Err(e) => {
                        println!("{}{} Connection failed: {}", prefix, status_error(), e);
                    }
                }
            } else {
                println!("{}{} Apple Calendar not configured", prefix, status_error());
            }
        }
        "google" => {
            if let Some(google) = config.calendar.google() {
                if google.access_token.expose().is_empty() {
                    println!(
                        "{}{} Google Calendar not authorized — run Configure credentials first",
                        prefix,
                        status_error()
                    );
                } else {
                    println!(
                        "{}{} Google Calendar authorized (calendar: {})",
                        prefix,
                        status_success(),
                        google.calendar_id
                    );
                }
            } else {
                println!(
                    "{}{} Google Calendar not configured",
                    prefix,
                    status_error()
                );
            }
        }
        "generic_caldav" => {
            if let Some(generic) = config.calendar.providers.iter().find_map(|p| match p {
                config::CalendarProviderConfig::GenericCalDav(c) => Some(c),
                _ => None,
            }) {
                let result = test_caldav_connection(
                    &generic.caldav_url,
                    &generic.username,
                    generic.password.expose(),
                )
                .await;

                match result {
                    Ok(()) => {
                        println!(
                            "{}{} Connection successful — {} ({})",
                            prefix,
                            status_success(),
                            generic.name,
                            generic.username
                        );
                    }
                    Err(e) => {
                        println!("{}{} Connection failed: {}", prefix, status_error(), e);
                    }
                }
            } else {
                println!("{}{} Generic CalDAV not configured", prefix, status_error());
            }
        }
        _ => {}
    }

    println!("{}", colorize(chars.vertical, BRAND));
    prompts::prompt_text("Press Enter to continue", Some(""), false)?;

    Ok(())
}

/// Execute the toggle enabled action for a calendar provider.
#[allow(clippy::single_match)]
pub(super) fn execute_toggle_enabled(config: &mut Config, provider_idx: usize) {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

    match provider.key {
        "apple" => {
            let apple = config.calendar.ensure_apple_mut();
            apple.enabled = !apple.enabled;
        }
        "google" => {
            let google = config.calendar.ensure_google_mut();
            google.enabled = !google.enabled;
        }
        _ => {}
    }
}

/// Execute the change calendar name action.
#[allow(clippy::single_match)]
pub(super) fn execute_change_calendar_name(config: &mut Config, provider_idx: usize) -> Result<()> {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

    match provider.key {
        "apple" => {
            let current_name = config
                .calendar
                .apple()
                .map(|a| a.calendar_name.clone())
                .unwrap_or_default();
            let current = if current_name.is_empty() {
                "Klyntbot"
            } else {
                &current_name
            };
            let name = prompts::prompt_text("Calendar name", Some(current), false)?;
            let apple = config.calendar.ensure_apple_mut();
            apple.calendar_name = name;
        }
        "google" => {
            let current_name = config
                .calendar
                .google()
                .map(|g| g.calendar_name.clone())
                .unwrap_or_default();
            let current = if current_name.is_empty() {
                "Klyntbot"
            } else {
                &current_name
            };
            let name = prompts::prompt_text("Calendar name", Some(current), false)?;
            let google = config.calendar.ensure_google_mut();
            google.calendar_name = name;
        }
        _ => {}
    }

    Ok(())
}

/// Execute the change sync interval action.
#[allow(clippy::single_match)]
pub(super) fn execute_change_sync_interval(config: &mut Config, provider_idx: usize) -> Result<()> {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

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

    let current_secs = get_sync_interval_for_provider(config, provider.key);
    let interval_default = match current_secs {
        0..=60 => 0,
        61..=300 => 1,
        301..=900 => 2,
        _ => 3,
    };

    let selected = prompts::prompt_select("Sync interval", &interval_options, interval_default)?;

    let new_interval = match selected {
        0 => 60,
        1 => 300,
        2 => 900,
        3 => 1800,
        _ => 300,
    };

    match provider.key {
        "apple" => {
            let apple = config.calendar.ensure_apple_mut();
            apple.sync_interval_secs = new_interval;
        }
        "google" => {
            let google = config.calendar.ensure_google_mut();
            google.sync_interval_secs = new_interval;
        }
        _ => {}
    }

    Ok(())
}

// ============================================================================
// Validation Helpers
// ============================================================================

/// Prompt for text input with validation.
pub(super) fn prompt_text_with_validation<F>(
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
        let input = prompts::prompt_text(label, default, required)?;

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
pub(super) fn validate_email(email: &str) -> Result<(), String> {
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

// ============================================================================
// Public Entry Point
// ============================================================================

/// Run the calendar configuration wizard step.
///
/// Presents an interactive expand-in-place menu for calendar provider
/// management, similar to the channels step.
pub async fn configure_calendars(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    println!(
        "{} {} Sync your calendar with klyntbot.",
        colorize(chars.vertical, BRAND),
        colorize("Calendar", BOLD)
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "Configure calendar integrations for todo sync.",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Show currently configured providers summary
    let configured_count = CALENDAR_PROVIDERS
        .iter()
        .filter(|p| is_provider_configured(config, p.key))
        .count();

    if configured_count > 0 {
        println!(
            "{} {} calendar(s) currently configured",
            colorize(chars.vertical, BRAND),
            colorize(&configured_count.to_string(), BOLD)
        );
        println!("{}", colorize(chars.vertical, BRAND));
    }

    // Run interactive menu (TTY) or fallback (non-TTY)
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        run_calendar_menu(config).await?;
    } else {
        run_calendar_menu_fallback(config).await?;
    }

    // Summary
    let configured: Vec<&str> = CALENDAR_PROVIDERS
        .iter()
        .filter(|p| is_provider_configured(config, p.key))
        .map(|p| p.name)
        .collect();

    if !configured.is_empty() {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} {} {} calendar(s) configured: {}",
            colorize(chars.vertical, BRAND),
            status_success(),
            configured.len(),
            configured.join(", ")
        );
    } else {
        println!(
            "{} {} No calendars configured. You can set them up later with:",
            colorize(chars.vertical, BRAND),
            colorize("Skipped.", DIM)
        );
        println!("{}", draw_step_line(&colorize("  klyntbot init", DIM)));
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use config::schema::Secret;
    use config::{AppleCalendarConfig, CalendarProviderConfig};

    #[test]
    fn test_provider_count() {
        assert_eq!(CALENDAR_PROVIDERS.len(), 3);
    }

    #[test]
    fn test_provider_keys_unique() {
        let keys: Vec<&str> = CALENDAR_PROVIDERS.iter().map(|p| p.key).collect();
        let mut unique_keys = keys.clone();
        unique_keys.sort();
        unique_keys.dedup();
        assert_eq!(keys.len(), unique_keys.len());
    }

    #[test]
    fn test_sub_action_count() {
        assert_eq!(SUB_ACTIONS.len(), 6);
    }

    #[test]
    fn test_sub_action_close_is_last() {
        assert_eq!(*SUB_ACTIONS.last().unwrap(), CalendarSubAction::Close);
    }

    #[test]
    fn test_sub_action_availability_unconfigured() {
        for action in SUB_ACTIONS {
            let available = action.is_available(false, false);
            match action {
                CalendarSubAction::ConfigureCredentials | CalendarSubAction::Close => {
                    assert!(available, "{:?} should be available", action);
                }
                _ => {
                    assert!(!available, "should not be available when unconfigured");
                }
            }
        }
    }

    #[test]
    fn test_sub_action_availability_configured() {
        for action in SUB_ACTIONS {
            assert!(action.is_available(true, true));
        }
    }

    #[test]
    fn test_is_provider_configured_empty() {
        let config = Config::default();
        assert!(!is_provider_configured(&config, "apple"));
    }

    #[test]
    fn test_is_provider_configured_with_credentials() {
        let mut config = Config::default();
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                username: "test@example.com".to_string(),
                password: Secret::new("test-password".to_string()),
                ..Default::default()
            }));
        assert!(is_provider_configured(&config, "apple"));
    }

    #[test]
    fn test_is_provider_configured_partial() {
        let mut config = Config::default();
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                username: "test@example.com".to_string(),
                // No password
                ..Default::default()
            }));
        assert!(!is_provider_configured(&config, "apple"));
    }

    #[test]
    fn test_is_provider_enabled() {
        let mut config = Config::default();
        assert!(!is_provider_enabled(&config, "apple"));
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                enabled: true,
                ..Default::default()
            }));
        assert!(is_provider_enabled(&config, "apple"));
    }

    #[test]
    fn test_is_provider_configured_unknown() {
        let config = Config::default();
        assert!(!is_provider_configured(&config, "outlook"));
        assert!(!is_provider_enabled(&config, "outlook"));
    }

    #[test]
    fn test_mask_provider_credentials() {
        let mut config = Config::default();
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                username: "user@example.com".to_string(),
                ..Default::default()
            }));
        assert_eq!(
            mask_provider_credentials(&config, "apple"),
            "user@example.com"
        );
    }

    #[test]
    fn test_format_interval() {
        assert_eq!(format_interval(60), "1 min");
        assert_eq!(format_interval(300), "5 min");
        assert_eq!(format_interval(900), "15 min");
        assert_eq!(format_interval(1800), "30 min");
        assert_eq!(format_interval(3600), "30 min");
    }

    #[test]
    fn test_get_provider_status_description() {
        let mut config = Config::default();
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                calendar_name: "My Calendar".to_string(),
                sync_interval_secs: 300,
                ..Default::default()
            }));
        let desc = get_provider_status_description(&config, "apple");
        assert!(desc.contains("My Calendar"));
        assert!(desc.contains("5 min"));
    }

    #[test]
    fn test_menu_state_new() {
        let menu = CalendarMenuState::new();
        assert_eq!(menu.cursor, 0);
        assert_eq!(menu.expanded, None);
        assert_eq!(menu.sub_cursor, 0);
        assert!(!menu.in_sub_menu);
    }

    #[test]
    fn test_menu_state_total_items() {
        let menu = CalendarMenuState::new();
        assert_eq!(menu.total_main_items(), CALENDAR_PROVIDERS.len() + 1);
    }

    #[test]
    fn test_menu_state_is_on_done() {
        let mut menu = CalendarMenuState::new();
        assert!(!menu.is_on_done());
        menu.cursor = CALENDAR_PROVIDERS.len();
        assert!(menu.is_on_done());
    }

    #[test]
    fn test_sub_action_labels_unconfigured() {
        let config = Config::default();
        let provider = &CALENDAR_PROVIDERS[0];

        let label = CalendarSubAction::ConfigureCredentials.label(&config, provider);
        assert_eq!(label, "Configure credentials");

        let label = CalendarSubAction::Close.label(&config, provider);
        assert_eq!(label, "Close");
    }

    #[test]
    fn test_sub_action_labels_configured() {
        let mut config = Config::default();
        config
            .calendar
            .providers
            .push(CalendarProviderConfig::Apple(AppleCalendarConfig {
                username: "user@example.com".to_string(),
                password: Secret::new("pass".to_string()),
                enabled: true,
                calendar_name: "Klyntbot".to_string(),
                sync_interval_secs: 300,
                ..Default::default()
            }));
        let provider = &CALENDAR_PROVIDERS[0];

        let label = CalendarSubAction::ConfigureCredentials.label(&config, provider);
        assert!(label.contains("user@example.com"));

        let label = CalendarSubAction::ToggleEnabled.label(&config, provider);
        assert!(label.contains("✓ enabled"));

        let label = CalendarSubAction::ChangeCalendarName.label(&config, provider);
        assert!(label.contains("Klyntbot"));

        let label = CalendarSubAction::ChangeSyncInterval.label(&config, provider);
        assert!(label.contains("5 min"));
    }

    #[test]
    fn test_toggle_enabled() {
        let mut config = Config::default();
        assert!(!is_provider_enabled(&config, "apple"));
        execute_toggle_enabled(&mut config, 0);
        assert!(is_provider_enabled(&config, "apple"));
        execute_toggle_enabled(&mut config, 0);
        assert!(!is_provider_enabled(&config, "apple"));
    }

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
