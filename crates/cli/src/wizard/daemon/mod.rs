//! Daemon/service setup wizard step.
//!
//! Generates platform-specific service configurations:
//! - systemd unit files (Linux) — user-level by default, system-level optional
//! - launchd plist files (macOS)
//! - Windows service wrapper guidance
//!
//! Also handles gateway (HTTP server) port configuration.

mod platform;

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts;

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
        match configure_daemon(&mut state.config)? {
            true => Ok(StepResult::Next),
            false => Ok(StepResult::Skip),
        }
    }
}

// ============================================================================
// Core logic
// ============================================================================

/// Run the daemon setup wizard step.
/// Returns true if daemon was configured, false if skipped.
pub fn configure_daemon(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    let wants_daemon = prompts::prompt_yes_no("Set up klyntbot as a background service?", false)?;
    if !wants_daemon {
        println!(
            "{} {} Skipping daemon setup {}",
            colorize(chars.vertical, BRAND),
            status_disabled(),
            colorize("(run manually with: klyntbot serve)", DIM)
        );
        return Ok(false);
    }

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
            return Ok(false);
        }
    }

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Daemon configuration complete",
        colorize(chars.vertical, BRAND),
        status_success()
    );

    Ok(true)
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
}
