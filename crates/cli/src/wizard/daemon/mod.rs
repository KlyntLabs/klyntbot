//! Daemon/service setup wizard step.
//!
//! Generates platform-specific service configurations:
//! - systemd unit files (Linux) — user-level by default, system-level optional
//! - launchd plist files (macOS)
//! - Windows service wrapper guidance
//!
//! Also handles gateway (HTTP server) port configuration.
//!
//! Uses a raw-mode interactive menu (not prompt_yes_no) so that
//! Esc / Back navigation works consistently with other wizard steps.

mod platform;

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use crossterm::{event::KeyCode, terminal};

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts;
use super::ui::{erase_lines, read_key, MenuOutcome};

// Re-export public APIs
pub use platform::{
    check_service_status, generate_launchd_plist, generate_systemd_system_unit,
    generate_systemd_user_unit, ServiceStatus,
};

use platform::{
    detect_platform, generate_launchd, generate_systemd, show_windows_guidance, Platform,
};

// ============================================================================
// WizardModule implementation
// ============================================================================

/// Daemon setup module for the wizard runner.
pub struct DaemonModule;

impl WizardModule for DaemonModule {
    fn name(&self) -> &str {
        "Background Service"
    }

    fn description(&self) -> &str {
        "Install klyntbot as a system service (auto-start on boot)"
    }

    fn is_required(&self) -> bool {
        false
    }

    fn is_applicable(&self, _state: &WizardState) -> bool {
        // Show on macOS, Linux, and Windows — skip on unknown/container
        let platform = detect_platform();
        !matches!(platform, Platform::Unknown)
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let can_go_back = state.current_step > 1;
        let outcome = if io::stdin().is_terminal() && io::stdout().is_terminal() {
            run_daemon_menu(&mut state.config, can_go_back)?
        } else {
            run_daemon_menu_fallback(&mut state.config, can_go_back)?
        };

        match outcome {
            MenuOutcome::Back => Ok(StepResult::Back),
            MenuOutcome::Done => Ok(StepResult::Next),
        }
    }
}

// ============================================================================
// Menu items
// ============================================================================

/// Items shown in the daemon menu.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DaemonItem {
    SetUp,
    Back,
    Skip,
}

fn menu_items(can_go_back: bool) -> Vec<DaemonItem> {
    let mut items = vec![DaemonItem::SetUp];
    if can_go_back {
        items.push(DaemonItem::Back);
    }
    items.push(DaemonItem::Skip);
    items
}

fn item_label(item: DaemonItem, selected: bool) -> String {
    match item {
        DaemonItem::SetUp => {
            let label = "Set up background service";
            if selected {
                colorize(label, BOLD)
            } else {
                label.to_string()
            }
        }
        DaemonItem::Back => {
            let label = "← Back";
            if selected {
                colorize(label, BOLD)
            } else {
                label.to_string()
            }
        }
        DaemonItem::Skip => {
            let label = "Skip";
            if selected {
                colorize(label, BOLD)
            } else {
                label.to_string()
            }
        }
    }
}

fn item_icon(item: DaemonItem) -> String {
    match item {
        DaemonItem::SetUp => colorize("●", BRAND),
        DaemonItem::Back => colorize("◁", DIM),
        DaemonItem::Skip => colorize("○", DIM),
    }
}

// ============================================================================
// Interactive menu (raw mode)
// ============================================================================

/// Render the daemon menu. Returns total lines rendered.
fn render_daemon_menu(
    out: &mut impl Write,
    items: &[DaemonItem],
    cursor: usize,
    chars: &BoxChars,
) -> Result<usize> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let mut lines = 0;

    for (i, &item) in items.iter().enumerate() {
        let is_cursor = cursor == i;
        let pointer = if is_cursor {
            colorize("❯", BRAND)
        } else {
            " ".to_string()
        };

        write!(
            out,
            "{}{} {} {}\r\n",
            prefix,
            pointer,
            item_icon(item),
            item_label(item, is_cursor)
        )?;
        lines += 1;
    }

    out.flush()?;
    Ok(lines)
}

fn render_daemon_hint(out: &mut impl Write, can_go_back: bool, chars: &BoxChars) -> Result<()> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let hint = if can_go_back {
        "↑/↓ navigate · Enter select · Esc back"
    } else {
        "↑/↓ navigate · Enter select"
    };
    write!(out, "{}{}\r\n", prefix, colorize(hint, DIM))?;
    out.flush()?;
    Ok(())
}

/// Run the interactive daemon menu in raw mode.
fn run_daemon_menu(config: &mut Config, can_go_back: bool) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let items = menu_items(can_go_back);
    let mut cursor: usize = 0;
    let mut out = io::stdout();

    terminal::enable_raw_mode()?;
    let mut list_lines = render_daemon_menu(&mut out, &items, cursor, chars)?;
    render_daemon_hint(&mut out, can_go_back, chars)?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if cursor > 0 {
                    cursor -= 1;
                    // Erase and re-render
                    let total = list_lines + 1;
                    for _ in 0..total {
                        write!(out, "\x1b[A\x1b[2K")?;
                    }
                    list_lines = render_daemon_menu(&mut out, &items, cursor, chars)?;
                    render_daemon_hint(&mut out, can_go_back, chars)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if cursor < items.len() - 1 {
                    cursor += 1;
                    let total = list_lines + 1;
                    for _ in 0..total {
                        write!(out, "\x1b[A\x1b[2K")?;
                    }
                    list_lines = render_daemon_menu(&mut out, &items, cursor, chars)?;
                    render_daemon_hint(&mut out, can_go_back, chars)?;
                }
            }
            KeyCode::Enter => {
                let selected = items[cursor];
                terminal::disable_raw_mode()?;
                erase_lines(list_lines + 1)?;

                match selected {
                    DaemonItem::Back => return Ok(MenuOutcome::Back),
                    DaemonItem::Skip => {
                        println!(
                            "{} {} Skipping daemon setup {}",
                            colorize(chars.vertical, BRAND),
                            status_disabled(),
                            colorize("(run manually with: klyntbot serve)", DIM)
                        );
                        return Ok(MenuOutcome::Done);
                    }
                    DaemonItem::SetUp => {
                        // Run the setup flow in cooked mode
                        run_daemon_setup(config)?;
                        return Ok(MenuOutcome::Done);
                    }
                }
            }
            KeyCode::Esc if can_go_back => {
                terminal::disable_raw_mode()?;
                erase_lines(list_lines + 1)?;
                return Ok(MenuOutcome::Back);
            }
            _ => {}
        }
    }
}

// ============================================================================
// Non-TTY fallback
// ============================================================================

/// Simplified menu for non-TTY environments.
fn run_daemon_menu_fallback(config: &mut Config, can_go_back: bool) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let items = menu_items(can_go_back);

    println!("{}", colorize(chars.vertical, BRAND));
    for (i, &item) in items.iter().enumerate() {
        println!(
            "{}  {}. {}",
            colorize(chars.vertical, BRAND),
            i + 1,
            item_label(item, false)
        );
    }
    println!("{}", colorize(chars.vertical, BRAND));

    let choice = prompts::prompt_text("Enter number", None, true)?;
    let idx = match choice.parse::<usize>() {
        Ok(n) if n > 0 && n <= items.len() => n - 1,
        _ => return Ok(MenuOutcome::Done), // default to skip
    };

    match items[idx] {
        DaemonItem::Back => Ok(MenuOutcome::Back),
        DaemonItem::Skip => {
            println!(
                "{} {} Skipping daemon setup {}",
                colorize(chars.vertical, BRAND),
                status_disabled(),
                colorize("(run manually with: klyntbot serve)", DIM)
            );
            Ok(MenuOutcome::Done)
        }
        DaemonItem::SetUp => {
            run_daemon_setup(config)?;
            Ok(MenuOutcome::Done)
        }
    }
}

// ============================================================================
// Core setup logic (cooked mode)
// ============================================================================

/// Run the daemon setup flow (gateway port + platform service generation).
fn run_daemon_setup(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    println!("{}", colorize(chars.vertical, BRAND));

    // Step 1: Configure gateway port
    configure_gateway(config)?;

    // Step 2: Generate service file
    let platform = detect_platform();
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} Detected platform: {}",
        colorize(chars.vertical, BRAND),
        colorize(platform.name(), BOLD)
    );

    match platform {
        Platform::MacOS => generate_launchd(config)?,
        Platform::Linux => generate_systemd(config)?,
        Platform::Windows => show_windows_guidance()?,
        Platform::Unknown => {
            println!(
                "{} {} Unsupported platform for automatic service setup",
                colorize(chars.vertical, BRAND),
                status_warning()
            );
            println!(
                "{}",
                draw_step_line(&colorize("Run manually: klyntbot serve", DIM))
            );
            return Ok(());
        }
    }

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Daemon configuration complete",
        colorize(chars.vertical, BRAND),
        status_success()
    );

    Ok(())
}

/// Configure gateway host and port
fn configure_gateway(config: &mut Config) -> Result<()> {
    println!(
        "  Current gateway: {}:{}",
        colorize(&config.gateway.host, DIM),
        colorize(&config.gateway.port.to_string(), BOLD)
    );

    let modify = prompts::prompt_yes_no("  Change gateway port?", false)?;
    if !modify {
        return Ok(());
    }

    let port_str = config.gateway.port.to_string();
    loop {
        let input = prompts::prompt_text("  Port", Some(&port_str), false)?;

        if input == port_str || input.is_empty() {
            return Ok(());
        }

        match input.parse::<u16>() {
            Ok(port) if port >= 1024 => {
                config.gateway.port = port;
                println!("  {} Gateway port set to {}", status_success(), port);
                return Ok(());
            }
            Ok(_) => {
                println!(
                    "  {}",
                    colorize("Port must be >= 1024 (non-privileged)", ERROR)
                );
            }
            Err(_) => {
                println!("  {}", colorize("Please enter a valid port number", ERROR));
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_platform() {
        let platform = detect_platform();
        // Should return a valid variant on any platform
        let _ = platform.name();

        #[cfg(target_os = "macos")]
        assert_eq!(platform, Platform::MacOS);

        #[cfg(target_os = "linux")]
        assert_eq!(platform, Platform::Linux);

        #[cfg(target_os = "windows")]
        assert_eq!(platform, Platform::Windows);
    }

    #[test]
    fn test_platform_names() {
        assert_eq!(Platform::MacOS.name(), "macOS (launchd)");
        assert_eq!(Platform::Linux.name(), "Linux (systemd)");
        assert_eq!(Platform::Windows.name(), "Windows");
        assert_eq!(Platform::Unknown.name(), "Unknown");
    }

    #[test]
    fn test_service_status_display() {
        assert!(!ServiceStatus::Running.display().is_empty());
        assert!(!ServiceStatus::Stopped.display().is_empty());
        assert!(!ServiceStatus::NotInstalled.display().is_empty());
        assert!(!ServiceStatus::Unknown.display().is_empty());
    }

    #[test]
    fn test_generate_launchd_plist_contains_required_fields() {
        let plist = generate_launchd_plist("/usr/local/bin/klyntbot", 18790);

        assert!(plist.contains("com.klyntbot.agent"));
        assert!(plist.contains("/usr/local/bin/klyntbot"));
        assert!(plist.contains("serve"));
        assert!(plist.contains("18790"));
        assert!(plist.contains("RunAtLoad"));
        assert!(plist.contains("KeepAlive"));
        assert!(plist.contains("StandardOutPath"));
        assert!(plist.contains("StandardErrorPath"));
        assert!(plist.contains("HOME"));
    }

    #[test]
    fn test_generate_launchd_plist_custom_port() {
        let plist = generate_launchd_plist("/opt/klyntbot", 9999);

        assert!(plist.contains("/opt/klyntbot"));
        assert!(plist.contains("9999"));
        assert!(!plist.contains("18790"));
    }

    #[test]
    fn test_generate_systemd_user_unit_contains_required_fields() {
        let unit = generate_systemd_user_unit("/usr/local/bin/klyntbot", 18790);

        assert!(unit.contains("/usr/local/bin/klyntbot"));
        assert!(unit.contains("serve --port 18790"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("default.target")); // user-level target
        assert!(!unit.contains("User=")); // user-level doesn't need User=
        assert!(!unit.contains("ProtectHome")); // no hardening in user mode
    }

    #[test]
    fn test_generate_systemd_system_unit_contains_required_fields() {
        let unit = generate_systemd_system_unit("/usr/local/bin/klyntbot", 18790);

        assert!(unit.contains("/usr/local/bin/klyntbot"));
        assert!(unit.contains("serve --port 18790"));
        assert!(unit.contains("User="));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("multi-user.target")); // system-level target
        assert!(unit.contains("NoNewPrivileges=true")); // hardening
        assert!(unit.contains("ProtectSystem=strict"));
        assert!(unit.contains("ReadWritePaths="));
    }

    #[test]
    fn test_generate_systemd_user_unit_custom_port() {
        let unit = generate_systemd_user_unit("/opt/klyntbot", 8080);

        assert!(unit.contains("/opt/klyntbot"));
        assert!(unit.contains("serve --port 8080"));
        assert!(!unit.contains("18790"));
    }

    #[test]
    fn test_daemon_module_metadata() {
        let module = DaemonModule;
        assert_eq!(module.name(), "Background Service");
        assert!(!module.is_required());
    }

    #[test]
    fn test_daemon_module_is_applicable() {
        let module = DaemonModule;
        let state = WizardState::new();

        // Should be applicable on macOS, Linux, or Windows
        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        assert!(module.is_applicable(&state));
    }

    #[test]
    fn test_launchd_plist_valid_xml() {
        let plist = generate_launchd_plist("/usr/local/bin/klyntbot", 18790);

        // Basic XML structure checks
        assert!(plist.starts_with("<?xml version=\"1.0\""));
        assert!(plist.contains("<!DOCTYPE plist"));
        assert!(plist.contains("<plist version=\"1.0\">"));
        assert!(plist.ends_with("</plist>"));
    }

    #[test]
    fn test_systemd_unit_valid_structure() {
        let unit = generate_systemd_user_unit("/usr/bin/klyntbot", 18790);

        // Must have all three INI sections
        assert!(unit.contains("[Unit]"));
        assert!(unit.contains("[Service]"));
        assert!(unit.contains("[Install]"));
    }

    #[test]
    fn test_service_status_equality() {
        assert_eq!(ServiceStatus::Running, ServiceStatus::Running);
        assert_eq!(ServiceStatus::Stopped, ServiceStatus::Stopped);
        assert_eq!(ServiceStatus::NotInstalled, ServiceStatus::NotInstalled);
        assert_eq!(ServiceStatus::Unknown, ServiceStatus::Unknown);
        assert_ne!(ServiceStatus::Running, ServiceStatus::Stopped);
    }

    #[test]
    fn test_menu_items_with_back() {
        let items = menu_items(true);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], DaemonItem::SetUp);
        assert_eq!(items[1], DaemonItem::Back);
        assert_eq!(items[2], DaemonItem::Skip);
    }

    #[test]
    fn test_menu_items_without_back() {
        let items = menu_items(false);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], DaemonItem::SetUp);
        assert_eq!(items[1], DaemonItem::Skip);
    }
}
