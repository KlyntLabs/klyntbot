//! Daemon/service setup wizard step.
//!
//! Generates platform-specific service configurations:
//! - systemd unit files (Linux) — user-level by default, system-level optional
//! - launchd plist files (macOS)
//! - Windows service wrapper guidance
//!
//! Also handles gateway (HTTP server) port configuration.

use std::path::PathBuf;

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts;

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
    let wants_daemon = prompts::prompt_yes_no(
        "Would you like to set up klyntbot as a background service?",
        false,
    )?;
    if !wants_daemon {
        println!(
            "\n  {} Skipping daemon setup {}",
            status_disabled(),
            colorize("(run manually with: klyntbot serve)", DIM)
        );
        return Ok(false);
    }

    println!();

    // Step 1: Configure gateway port
    configure_gateway(config)?;

    // Step 2: Generate service file
    let platform = detect_platform();
    println!(
        "\n  Detected platform: {}",
        colorize(platform.name(), BOLD)
    );

    match platform {
        Platform::MacOS => generate_launchd(config)?,
        Platform::Linux => generate_systemd(config)?,
        Platform::Windows => show_windows_guidance()?,
        Platform::Unknown => {
            println!(
                "  {} Unsupported platform for automatic service setup",
                status_warning()
            );
            println!(
                "  {}",
                colorize("Run manually: klyntbot serve", DIM)
            );
            return Ok(false);
        }
    }

    println!(
        "\n  {} Daemon configuration complete",
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
                println!(
                    "  {}",
                    colorize("Please enter a valid port number", ERROR)
                );
            }
        }
    }
}

// ============================================================================
// Platform detection
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
enum Platform {
    MacOS,
    Linux,
    Windows,
    Unknown,
}

impl Platform {
    fn name(&self) -> &str {
        match self {
            Self::MacOS => "macOS (launchd)",
            Self::Linux => "Linux (systemd)",
            Self::Windows => "Windows",
            Self::Unknown => "Unknown",
        }
    }
}

fn detect_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOS
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Unknown
    }
}

// ============================================================================
// macOS launchd
// ============================================================================

/// Generate and optionally install a launchd plist (macOS)
fn generate_launchd(config: &Config) -> Result<()> {
    let binary_path =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/local/bin/klyntbot"));

    let plist_content = generate_launchd_plist(
        &binary_path.display().to_string(),
        config.gateway.port,
    );

    let plist_dir = dirs::home_dir()
        .map(|h| h.join("Library/LaunchAgents"))
        .unwrap_or_else(|| PathBuf::from("~/Library/LaunchAgents"));

    let plist_path = plist_dir.join("com.klyntbot.agent.plist");

    println!("\n  Generated launchd plist:");
    println!(
        "  {}",
        colorize(&format!("  {}", plist_path.display()), DIM)
    );

    let install = prompts::prompt_yes_no("\n  Write plist file and enable service?", false)?;
    if install {
        std::fs::create_dir_all(&plist_dir)?;
        std::fs::write(&plist_path, &plist_content)?;
        println!(
            "  {} Plist written to {}",
            status_success(),
            plist_path.display()
        );

        print_launchd_management(&plist_path);

        let start_now = prompts::prompt_yes_no("\n  Start service now?", true)?;
        if start_now {
            let status = std::process::Command::new("launchctl")
                .args(["load", &plist_path.display().to_string()])
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!(
                        "  {} Service started",
                        status_success()
                    );
                }
                Ok(_) => {
                    println!(
                        "  {} Failed to start service. Try manually:",
                        status_warning()
                    );
                    println!(
                        "  {}",
                        colorize(
                            &format!("  launchctl load {}", plist_path.display()),
                            DIM,
                        )
                    );
                }
                Err(e) => {
                    println!(
                        "  {} Could not run launchctl: {}",
                        status_warning(),
                        e
                    );
                }
            }
        }
    } else {
        println!(
            "\n  {} Plist content (copy manually):\n",
            status_success()
        );
        println!("{}", colorize(&plist_content, DIM));
    }

    Ok(())
}

/// Print launchd management commands
fn print_launchd_management(plist_path: &std::path::Path) {
    let path_str = plist_path.display().to_string();

    println!("\n  {}", colorize("To manage the service:", BOLD));
    println!(
        "  {}",
        colorize(&format!("  Load:    launchctl load {}", path_str), DIM)
    );
    println!(
        "  {}",
        colorize(&format!("  Unload:  launchctl unload {}", path_str), DIM)
    );
    println!(
        "  {}",
        colorize("  Status:  launchctl list | grep klyntbot", DIM)
    );
    println!(
        "  {}",
        colorize("  Logs:    tail -f /tmp/klyntbot.stdout.log", DIM)
    );
    println!(
        "\n  {}",
        colorize("To uninstall:", BOLD)
    );
    println!(
        "  {}",
        colorize(
            &format!("  launchctl unload {} && rm {}", path_str, path_str),
            DIM,
        )
    );
}

/// Generate launchd plist content (public for testing)
pub fn generate_launchd_plist(binary_path: &str, port: u16) -> String {
    let home = dirs::home_dir()
        .map(|h| h.display().to_string())
        .unwrap_or_else(|| "/Users/unknown".to_string());

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.klyntbot.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary_path}</string>
        <string>serve</string>
        <string>--port</string>
        <string>{port}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/klyntbot.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/klyntbot.stderr.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOME</key>
        <string>{home}</string>
    </dict>
</dict>
</plist>"#
    )
}

// ============================================================================
// Linux systemd
// ============================================================================

/// Systemd installation mode
#[derive(Debug, Clone, Copy, PartialEq)]
enum SystemdMode {
    /// User-level service (~/.config/systemd/user/) — no sudo required
    User,
    /// System-level service (/etc/systemd/system/) — requires sudo
    System,
}

/// Generate and optionally install a systemd unit file (Linux)
fn generate_systemd(config: &Config) -> Result<()> {
    let binary_path =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/local/bin/klyntbot"));

    // Offer user-level vs system-level
    let mode = select_systemd_mode()?;

    let (unit_content, unit_path) = match mode {
        SystemdMode::User => {
            let content = generate_systemd_user_unit(
                &binary_path.display().to_string(),
                config.gateway.port,
            );
            let path = dirs::home_dir()
                .map(|h| h.join(".config/systemd/user/klyntbot.service"))
                .unwrap_or_else(|| PathBuf::from("~/.config/systemd/user/klyntbot.service"));
            (content, path)
        }
        SystemdMode::System => {
            let content = generate_systemd_system_unit(
                &binary_path.display().to_string(),
                config.gateway.port,
            );
            let path = PathBuf::from("/etc/systemd/system/klyntbot.service");
            (content, path)
        }
    };

    println!("\n  Generated systemd unit file:");
    println!(
        "  {}",
        colorize(&format!("  {}", unit_path.display()), DIM)
    );

    match mode {
        SystemdMode::User => {
            let install = prompts::prompt_yes_no("\n  Write unit file and enable service?", true)?;
            if install {
                if let Some(parent) = unit_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&unit_path, &unit_content)?;
                println!(
                    "  {} Unit written to {}",
                    status_success(),
                    unit_path.display()
                );

                // Reload and enable
                let reload = std::process::Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .status();
                if let Ok(s) = reload {
                    if s.success() {
                        println!("  {} systemd daemon reloaded", status_success());
                    }
                }

                let enable = std::process::Command::new("systemctl")
                    .args(["--user", "enable", "klyntbot"])
                    .status();
                if let Ok(s) = enable {
                    if s.success() {
                        println!("  {} Service enabled (auto-start on login)", status_success());
                    }
                }

                print_systemd_management(SystemdMode::User);

                let start_now = prompts::prompt_yes_no("\n  Start service now?", true)?;
                if start_now {
                    let status = std::process::Command::new("systemctl")
                        .args(["--user", "start", "klyntbot"])
                        .status();
                    match status {
                        Ok(s) if s.success() => {
                            println!("  {} Service started", status_success());
                        }
                        _ => {
                            println!(
                                "  {} Start manually: systemctl --user start klyntbot",
                                status_warning()
                            );
                        }
                    }
                }
            } else {
                println!("\n  Unit file content:\n");
                println!("{}", colorize(&unit_content, DIM));
            }
        }
        SystemdMode::System => {
            println!(
                "\n  {} System-level systemd requires sudo",
                status_warning()
            );

            let show = prompts::prompt_yes_no("  Show unit file content?", true)?;
            if show {
                println!("\n{}", colorize(&unit_content, DIM));
            }

            print_systemd_management(SystemdMode::System);

            // Save locally for manual install
            let save_local = prompts::prompt_yes_no(
                "\n  Save unit file to ~/.klyntbot/klyntbot.service?",
                true,
            )?;
            if save_local {
                let local_path = config::config_dir()?.join("klyntbot.service");
                std::fs::write(&local_path, &unit_content)?;
                println!(
                    "  {} Saved to {}",
                    status_success(),
                    local_path.display()
                );
                println!(
                    "  {}",
                    colorize(
                        &format!(
                            "  Install: sudo cp {} {}",
                            local_path.display(),
                            unit_path.display()
                        ),
                        DIM,
                    )
                );
            }
        }
    }

    Ok(())
}

/// Let user choose between user-level and system-level systemd
fn select_systemd_mode() -> Result<SystemdMode> {
    let options = [
        prompts::SelectOption {
            label: "User service",
            description: "No sudo required, starts on login (~/.config/systemd/user/)",
        },
        prompts::SelectOption {
            label: "System service",
            description: "Requires sudo, starts on boot (/etc/systemd/system/)",
        },
    ];

    let idx = prompts::prompt_select("  Systemd installation mode", &options, 0)?;
    Ok(match idx {
        0 => SystemdMode::User,
        _ => SystemdMode::System,
    })
}

/// Print systemd management commands
fn print_systemd_management(mode: SystemdMode) {
    let flag = match mode {
        SystemdMode::User => " --user",
        SystemdMode::System => "",
    };
    let prefix = match mode {
        SystemdMode::User => "",
        SystemdMode::System => "sudo ",
    };

    println!("\n  {}", colorize("To manage the service:", BOLD));
    println!(
        "  {}",
        colorize(
            &format!("  Start:   {prefix}systemctl{flag} start klyntbot"),
            DIM,
        )
    );
    println!(
        "  {}",
        colorize(
            &format!("  Stop:    {prefix}systemctl{flag} stop klyntbot"),
            DIM,
        )
    );
    println!(
        "  {}",
        colorize(
            &format!("  Status:  {prefix}systemctl{flag} status klyntbot"),
            DIM,
        )
    );
    println!(
        "  {}",
        colorize(
            &format!("  Logs:    {prefix}journalctl{flag} -u klyntbot -f"),
            DIM,
        )
    );

    println!(
        "\n  {}",
        colorize("To uninstall:", BOLD)
    );
    println!(
        "  {}",
        colorize(
            &format!("  {prefix}systemctl{flag} disable --now klyntbot"),
            DIM,
        )
    );
    match mode {
        SystemdMode::User => {
            println!(
                "  {}",
                colorize("  rm ~/.config/systemd/user/klyntbot.service", DIM)
            );
        }
        SystemdMode::System => {
            println!(
                "  {}",
                colorize("  sudo rm /etc/systemd/system/klyntbot.service", DIM)
            );
        }
    }
}

/// Generate user-level systemd unit content (public for testing)
pub fn generate_systemd_user_unit(binary_path: &str, port: u16) -> String {
    format!(
        r#"[Unit]
Description=Klyntbot AI Agent Service
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={binary_path} serve --port {port}
Restart=on-failure
RestartSec=10
Environment=RUST_LOG=info

[Install]
WantedBy=default.target"#
    )
}

/// Generate system-level systemd unit content (public for testing)
pub fn generate_systemd_system_unit(binary_path: &str, port: u16) -> String {
    let user = std::env::var("USER").unwrap_or_else(|_| "klyntbot".to_string());
    let home = dirs::home_dir()
        .map(|h| h.display().to_string())
        .unwrap_or_else(|| format!("/home/{}", user));
    let config_dir = config::config_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_else(|_| format!("/home/{}/.klyntbot", user));

    format!(
        r#"[Unit]
Description=Klyntbot AI Agent Service
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={user}
ExecStart={binary_path} serve --port {port}
Restart=on-failure
RestartSec=5
Environment=HOME={home}

# Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths={config_dir}

[Install]
WantedBy=multi-user.target"#
    )
}

// ============================================================================
// Windows
// ============================================================================

/// Show Windows service guidance (no direct installation)
fn show_windows_guidance() -> Result<()> {
    println!("\n  {}", colorize("Windows service setup options:", BOLD));

    println!("\n  {}", colorize("Option 1: NSSM (recommended)", BOLD));
    println!(
        "  {}",
        colorize("  1. Download NSSM: https://nssm.cc/download", DIM)
    );
    println!(
        "  {}",
        colorize("  2. Run: nssm install klyntbot", DIM)
    );
    println!(
        "  {}",
        colorize("  3. Set path to klyntbot.exe, arguments: serve", DIM)
    );
    println!(
        "  {}",
        colorize("  4. Start: nssm start klyntbot", DIM)
    );

    println!(
        "\n  {}",
        colorize("Option 2: Task Scheduler", BOLD)
    );
    println!(
        "  {}",
        colorize("  1. Open Task Scheduler (taskschd.msc)", DIM)
    );
    println!(
        "  {}",
        colorize("  2. Create Basic Task > 'klyntbot'", DIM)
    );
    println!(
        "  {}",
        colorize("  3. Trigger: At startup", DIM)
    );
    println!(
        "  {}",
        colorize("  4. Action: Start klyntbot.exe serve", DIM)
    );

    println!("\n  {}", colorize("Option 3: sc.exe (advanced)", BOLD));
    println!(
        "  {}",
        colorize(
            "  sc create klyntbot binPath= \"C:\\path\\to\\klyntbot.exe serve\" start= auto",
            DIM,
        )
    );

    Ok(())
}

// ============================================================================
// Service status (public API for `klyntbot status`)
// ============================================================================

/// Check if klyntbot is running as a service
pub fn check_service_status() -> ServiceStatus {
    if cfg!(target_os = "macos") {
        check_launchd_status()
    } else if cfg!(target_os = "linux") {
        check_systemd_status()
    } else {
        ServiceStatus::Unknown
    }
}

/// Service status result
#[derive(Debug, PartialEq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    NotInstalled,
    Unknown,
}

impl ServiceStatus {
    pub fn display(&self) -> String {
        match self {
            Self::Running => format!("{} Service running", status_success()),
            Self::Stopped => format!("{} Service stopped", status_warning()),
            Self::NotInstalled => format!("{} Service not installed", status_disabled()),
            Self::Unknown => format!("{} Service status unknown", status_disabled()),
        }
    }
}

fn check_launchd_status() -> ServiceStatus {
    let output = std::process::Command::new("launchctl")
        .args(["list", "com.klyntbot.agent"])
        .output();

    match output {
        Ok(out) if out.status.success() => ServiceStatus::Running,
        Ok(_) => {
            let plist = dirs::home_dir()
                .map(|h| h.join("Library/LaunchAgents/com.klyntbot.agent.plist"))
                .unwrap_or_default();
            if plist.exists() {
                ServiceStatus::Stopped
            } else {
                ServiceStatus::NotInstalled
            }
        }
        Err(_) => ServiceStatus::Unknown,
    }
}

fn check_systemd_status() -> ServiceStatus {
    // Try user-level first (preferred), then system-level
    let user_output = std::process::Command::new("systemctl")
        .args(["--user", "is-active", "klyntbot"])
        .output();

    if let Ok(out) = user_output {
        let status = String::from_utf8_lossy(&out.stdout).trim().to_string();
        match status.as_str() {
            "active" => return ServiceStatus::Running,
            "inactive" | "failed" => return ServiceStatus::Stopped,
            _ => {}
        }
    }

    // Fall back to system-level check
    let sys_output = std::process::Command::new("systemctl")
        .args(["is-active", "klyntbot"])
        .output();

    match sys_output {
        Ok(out) => {
            let status = String::from_utf8_lossy(&out.stdout).trim().to_string();
            match status.as_str() {
                "active" => ServiceStatus::Running,
                "inactive" | "failed" => ServiceStatus::Stopped,
                _ => ServiceStatus::NotInstalled,
            }
        }
        Err(_) => ServiceStatus::Unknown,
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
