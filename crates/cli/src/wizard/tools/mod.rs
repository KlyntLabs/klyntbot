//! Tools configuration wizard step.
//!
//! Provides an expand-in-place interactive menu for:
//! - Security preset profiles (strict, balanced, permissive)
//! - Workspace restriction toggle
//! - Shell command allowlist editing
//! - Brave Search API key
//! - Execution timeout
//!
//! On reconfiguration, all fields show current values as defaults.

pub(crate) mod menus;

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

use super::prompts;

// ============================================================================
// Preset Types
// ============================================================================

/// Security preset profile for tools configuration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolsPreset {
    Strict,
    Balanced,
    Permissive,
}

impl ToolsPreset {
    fn description(&self) -> &str {
        match self {
            Self::Strict => "Workspace-locked, allowlisted commands only, 30s timeout",
            Self::Balanced => "Workspace-locked, deny dangerous commands, 60s timeout",
            Self::Permissive => "No workspace restriction, deny dangerous commands, 120s timeout",
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Strict => "Strict",
            Self::Balanced => "Balanced",
            Self::Permissive => "Permissive",
        }
    }
}

// ============================================================================
// Setting Metadata
// ============================================================================

pub(super) struct ToolSettingInfo {
    pub(super) key: &'static str,
    pub(super) name: &'static str,
}

pub(super) const TOOL_SETTINGS: &[ToolSettingInfo] = &[
    ToolSettingInfo {
        key: "preset",
        name: "Security Preset",
    },
    ToolSettingInfo {
        key: "workspace",
        name: "Workspace Restriction",
    },
    ToolSettingInfo {
        key: "allowlist",
        name: "Shell Allowlist",
    },
    ToolSettingInfo {
        key: "brave_api",
        name: "Brave Search API",
    },
    ToolSettingInfo {
        key: "timeout",
        name: "Execution Timeout",
    },
];

// ============================================================================
// Configuration Detection Helpers
// ============================================================================

/// Detect the current preset from config values.
pub(super) fn detect_preset(config: &Config) -> ToolsPreset {
    if config.tools.restrict_to_workspace
        && !config.tools.exec.allowed_commands.is_empty()
        && config.tools.exec.timeout <= 30
    {
        ToolsPreset::Strict
    } else if !config.tools.restrict_to_workspace && config.tools.exec.timeout >= 120 {
        ToolsPreset::Permissive
    } else {
        ToolsPreset::Balanced
    }
}

/// Get the icon and color for a setting based on current config state.
pub(super) fn get_setting_icon(config: &Config, key: &str) -> (&'static str, &'static str) {
    match key {
        "preset" => ("★", HIGHLIGHT),
        "workspace" => {
            if config.tools.restrict_to_workspace {
                ("✓", SUCCESS)
            } else {
                ("○", DIM)
            }
        }
        "allowlist" => {
            if config.tools.exec.allowed_commands.is_empty() {
                ("○", DIM)
            } else {
                ("★", HIGHLIGHT)
            }
        }
        "brave_api" => {
            if config.tools.web.brave_api_key.is_empty() {
                ("○", DIM)
            } else {
                ("✓", SUCCESS)
            }
        }
        "timeout" => ("○", DIM),
        _ => ("○", DIM),
    }
}

/// Get the status text for a setting.
pub(super) fn get_setting_status(config: &Config, key: &str) -> String {
    match key {
        "preset" => detect_preset(config).name().to_string(),
        "workspace" => {
            if config.tools.restrict_to_workspace {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            }
        }
        "allowlist" => {
            if config.tools.exec.allowed_commands.is_empty() {
                "deny-list mode".to_string()
            } else {
                format!("{} commands", config.tools.exec.allowed_commands.len())
            }
        }
        "brave_api" => {
            let key_val = config.tools.web.brave_api_key.expose();
            if key_val.is_empty() {
                "not configured".to_string()
            } else {
                format!("configured ({})", prompts::mask_secret(key_val))
            }
        }
        "timeout" => format!("{}s", config.tools.exec.timeout),
        _ => String::new(),
    }
}

// ============================================================================
// State Model and Sub-Actions
// ============================================================================

/// Actions available in expanded setting sub-menus.
#[derive(Clone, PartialEq)]
pub(super) enum ToolSubAction {
    SwitchPreset(ToolsPreset),
    ToggleWorkspace,
    EditAllowlist,
    SwitchAllowlistMode,
    ConfigureBraveKey,
    EditBraveKey,
    RemoveBraveKey,
    ChangeTimeout,
    Close,
}

impl ToolSubAction {
    pub(super) fn label(&self, config: &Config) -> String {
        match self {
            Self::SwitchPreset(preset) => {
                format!("{} ({})", preset.name(), preset.description())
            }
            Self::ToggleWorkspace => {
                if config.tools.restrict_to_workspace {
                    "Disable restriction".to_string()
                } else {
                    "Enable restriction".to_string()
                }
            }
            Self::EditAllowlist => {
                let cmds = &config.tools.exec.allowed_commands;
                if cmds.len() <= 5 {
                    format!("Edit commands [{}]", cmds.join(", "))
                } else {
                    let preview: Vec<&str> = cmds.iter().take(4).map(|s| s.as_str()).collect();
                    format!("Edit commands [{}, ...]", preview.join(", "))
                }
            }
            Self::SwitchAllowlistMode => {
                if config.tools.exec.allowed_commands.is_empty() {
                    "Switch to allowlist mode".to_string()
                } else {
                    "Switch to deny-list mode".to_string()
                }
            }
            Self::ConfigureBraveKey => "Configure API key".to_string(),
            Self::EditBraveKey => {
                let key = config.tools.web.brave_api_key.expose();
                format!("Edit API key ({})", prompts::mask_secret(key))
            }
            Self::RemoveBraveKey => "Remove API key".to_string(),
            Self::ChangeTimeout => "Change timeout".to_string(),
            Self::Close => "Close".to_string(),
        }
    }
}

/// Get the dynamic sub-actions for a setting based on current config state.
pub(super) fn get_sub_actions(config: &Config, setting_idx: usize) -> Vec<ToolSubAction> {
    let key = TOOL_SETTINGS[setting_idx].key;
    match key {
        "preset" => {
            let current = detect_preset(config);
            let mut actions = Vec::new();
            for preset in &[
                ToolsPreset::Strict,
                ToolsPreset::Balanced,
                ToolsPreset::Permissive,
            ] {
                if *preset != current {
                    actions.push(ToolSubAction::SwitchPreset(*preset));
                }
            }
            actions.push(ToolSubAction::Close);
            actions
        }
        "workspace" => {
            vec![ToolSubAction::ToggleWorkspace, ToolSubAction::Close]
        }
        "allowlist" => {
            if config.tools.exec.allowed_commands.is_empty() {
                vec![ToolSubAction::SwitchAllowlistMode, ToolSubAction::Close]
            } else {
                vec![
                    ToolSubAction::EditAllowlist,
                    ToolSubAction::SwitchAllowlistMode,
                    ToolSubAction::Close,
                ]
            }
        }
        "brave_api" => {
            if config.tools.web.brave_api_key.is_empty() {
                vec![ToolSubAction::ConfigureBraveKey, ToolSubAction::Close]
            } else {
                vec![
                    ToolSubAction::EditBraveKey,
                    ToolSubAction::RemoveBraveKey,
                    ToolSubAction::Close,
                ]
            }
        }
        "timeout" => {
            vec![ToolSubAction::ChangeTimeout, ToolSubAction::Close]
        }
        _ => vec![ToolSubAction::Close],
    }
}

/// State for the interactive tool permissions menu.
pub(super) struct ToolMenuState {
    pub(super) cursor: usize,
    pub(super) expanded: Option<usize>,
    pub(super) sub_cursor: usize,
    pub(super) in_sub_menu: bool,
}

impl ToolMenuState {
    pub(super) fn new() -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
        }
    }

    pub(super) fn total_main_items(&self) -> usize {
        TOOL_SETTINGS.len() + 1 // settings + "Done"
    }

    pub(super) fn is_on_done(&self) -> bool {
        self.cursor == TOOL_SETTINGS.len()
    }
}

// ============================================================================
// Sub-Action Executors
// ============================================================================

/// Apply a preset profile to the config.
pub(super) fn apply_preset(config: &mut Config, preset: ToolsPreset) {
    match preset {
        ToolsPreset::Strict => {
            config.tools.restrict_to_workspace = true;
            config.tools.exec.timeout = 30;
            config.tools.exec.allowed_commands = vec![
                "ls".to_string(),
                "cat".to_string(),
                "head".to_string(),
                "tail".to_string(),
                "grep".to_string(),
                "find".to_string(),
                "wc".to_string(),
                "echo".to_string(),
                "pwd".to_string(),
                "date".to_string(),
            ];
        }
        ToolsPreset::Balanced => {
            config.tools.restrict_to_workspace = true;
            config.tools.exec.timeout = 60;
            config.tools.exec.allowed_commands = Vec::new();
        }
        ToolsPreset::Permissive => {
            config.tools.restrict_to_workspace = false;
            config.tools.exec.timeout = 120;
            config.tools.exec.allowed_commands = Vec::new();
        }
    }
}

/// Execute the edit allowlist action (exits raw mode for input).
pub(super) fn execute_edit_allowlist(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{}",
        prefix,
        colorize("Enter commands one per line (empty line to finish):", DIM)
    );

    let mut commands: Vec<String> = config.tools.exec.allowed_commands.clone();

    if !commands.is_empty() {
        println!("{}Current: {}", prefix, commands.join(", "));
        let clear = prompts::prompt_yes_no("Clear existing and start fresh?", false)?;
        if clear {
            commands.clear();
        }
    }

    loop {
        print!("{}{} ", prefix, colorize("+", SUCCESS));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            break;
        }

        if commands.contains(&input) {
            println!("{}  {}", prefix, colorize("(already in list)", DIM));
            continue;
        }

        commands.push(input);
    }

    config.tools.exec.allowed_commands = commands;

    if config.tools.exec.allowed_commands.is_empty() {
        println!(
            "{}{} Allowlist cleared, using deny-list mode",
            prefix,
            status_success()
        );
    } else {
        println!(
            "{}{} Allowlist set: {}",
            prefix,
            status_success(),
            config.tools.exec.allowed_commands.join(", ")
        );
    }

    Ok(())
}

/// Execute the configure/edit brave API key action (exits raw mode for input).
pub(super) fn execute_configure_brave_key(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    let existing_key = config.tools.web.brave_api_key.expose().clone();

    if existing_key.is_empty() {
        println!(
            "{}Get yours at: {}",
            prefix,
            colorize("https://brave.com/search/api/", UNDERLINE)
        );
        let key = prompts::prompt_secret("Brave API Key", 10)?;
        config.tools.web.brave_api_key = config::schema::Secret::new(key);
        println!(
            "{}{} Brave Search API key configured",
            prefix,
            status_success()
        );
    } else {
        match prompts::prompt_secret_with_existing("Brave API Key", &existing_key, 10)? {
            Some(new_key) => {
                config.tools.web.brave_api_key = config::schema::Secret::new(new_key);
                println!(
                    "{}{} Brave Search API key updated",
                    prefix,
                    status_success()
                );
            }
            None => {
                println!("{}{} Brave API key unchanged", prefix, status_success());
            }
        }
    }

    Ok(())
}

/// Execute the change timeout action (exits raw mode for input).
pub(super) fn execute_change_timeout(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    let current = config.tools.exec.timeout.to_string();
    let input = prompts::prompt_text("Execution timeout (seconds)", Some(&current), true)?;

    match input.parse::<u64>() {
        Ok(secs) if (5..=600).contains(&secs) => {
            config.tools.exec.timeout = secs;
            println!("{}{} Timeout set to {}s", prefix, status_success(), secs);
        }
        Ok(_) => {
            println!(
                "{}{}",
                prefix,
                colorize(
                    "Timeout must be between 5 and 600 seconds, keeping current",
                    WARNING
                )
            );
        }
        Err(_) => {
            println!(
                "{}{}",
                prefix,
                colorize("Invalid number, keeping current timeout", WARNING)
            );
        }
    }

    Ok(())
}

// ============================================================================
// Public Entry Point
// ============================================================================

/// Run the tools configuration wizard step.
/// Returns true if tools were configured, false if skipped.
pub fn configure_tools(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    // Show current status summary
    let preset = detect_preset(config);
    println!(
        "{} Current: {} preset, timeout {}s, workspace restriction {}",
        colorize(chars.vertical, BRAND),
        colorize(preset.name(), BOLD),
        colorize(&config.tools.exec.timeout.to_string(), DIM),
        if config.tools.restrict_to_workspace {
            colorize("on", SUCCESS)
        } else {
            colorize("off", WARNING)
        }
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Run interactive menu (TTY) or fallback (non-TTY)
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        menus::run_tool_menu(config)?;
    } else {
        menus::run_tool_menu_fallback(config)?;
    }

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Tools configuration complete",
        colorize(chars.vertical, BRAND),
        status_success()
    );

    Ok(true)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use config::Config;

    // ========================================================================
    // Preset application tests
    // ========================================================================

    #[test]
    fn test_apply_preset_strict() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict);

        assert!(config.tools.restrict_to_workspace);
        assert_eq!(config.tools.exec.timeout, 30);
        assert!(!config.tools.exec.allowed_commands.is_empty());
        assert!(config
            .tools
            .exec
            .allowed_commands
            .contains(&"ls".to_string()));
        assert!(config
            .tools
            .exec
            .allowed_commands
            .contains(&"cat".to_string()));
    }

    #[test]
    fn test_apply_preset_balanced() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Balanced);

        assert!(config.tools.restrict_to_workspace);
        assert_eq!(config.tools.exec.timeout, 60);
        assert!(config.tools.exec.allowed_commands.is_empty());
    }

    #[test]
    fn test_apply_preset_permissive() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Permissive);

        assert!(!config.tools.restrict_to_workspace);
        assert_eq!(config.tools.exec.timeout, 120);
        assert!(config.tools.exec.allowed_commands.is_empty());
    }

    #[test]
    fn test_preset_descriptions() {
        assert!(!ToolsPreset::Strict.description().is_empty());
        assert!(!ToolsPreset::Balanced.description().is_empty());
        assert!(!ToolsPreset::Permissive.description().is_empty());
    }

    // ========================================================================
    // Detect preset tests
    // ========================================================================

    #[test]
    fn test_detect_preset_default_is_balanced() {
        let config = Config::default();
        assert_eq!(detect_preset(&config), ToolsPreset::Balanced);
    }

    #[test]
    fn test_detect_preset_strict() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict);
        assert_eq!(detect_preset(&config), ToolsPreset::Strict);
    }

    #[test]
    fn test_detect_preset_permissive() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Permissive);
        assert_eq!(detect_preset(&config), ToolsPreset::Permissive);
    }

    // ========================================================================
    // Strict preset detail tests
    // ========================================================================

    #[test]
    fn test_strict_preset_has_safe_commands_only() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict);

        let safe_commands = vec![
            "ls", "cat", "head", "tail", "grep", "find", "wc", "echo", "pwd", "date",
        ];
        assert_eq!(
            config.tools.exec.allowed_commands.len(),
            safe_commands.len()
        );

        for cmd in safe_commands {
            assert!(
                config
                    .tools
                    .exec
                    .allowed_commands
                    .contains(&cmd.to_string()),
                "Strict preset should include '{}'",
                cmd
            );
        }
    }

    #[test]
    fn test_strict_preset_does_not_include_dangerous_commands() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict);

        let dangerous = vec!["rm", "sudo", "chmod", "chown", "kill", "mkfs", "dd"];
        for cmd in dangerous {
            assert!(
                !config
                    .tools
                    .exec
                    .allowed_commands
                    .contains(&cmd.to_string()),
                "Strict preset should NOT include '{}'",
                cmd
            );
        }
    }

    // ========================================================================
    // Preset transitions tests
    // ========================================================================

    #[test]
    fn test_preset_transition_strict_to_permissive() {
        let mut config = Config::default();

        apply_preset(&mut config, ToolsPreset::Strict);
        assert!(config.tools.restrict_to_workspace);
        assert!(!config.tools.exec.allowed_commands.is_empty());

        apply_preset(&mut config, ToolsPreset::Permissive);
        assert!(!config.tools.restrict_to_workspace);
        assert!(config.tools.exec.allowed_commands.is_empty());
        assert_eq!(config.tools.exec.timeout, 120);
    }

    #[test]
    fn test_preset_transition_permissive_to_strict() {
        let mut config = Config::default();

        apply_preset(&mut config, ToolsPreset::Permissive);
        apply_preset(&mut config, ToolsPreset::Strict);

        assert!(config.tools.restrict_to_workspace);
        assert!(!config.tools.exec.allowed_commands.is_empty());
        assert_eq!(config.tools.exec.timeout, 30);
    }

    // ========================================================================
    // Config serialization after preset tests
    // ========================================================================

    #[test]
    fn test_preset_config_serialization_round_trip() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict);

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert!(loaded.tools.restrict_to_workspace);
        assert_eq!(loaded.tools.exec.timeout, 30);
        assert!(loaded
            .tools
            .exec
            .allowed_commands
            .contains(&"ls".to_string()));
    }

    #[test]
    fn test_preset_descriptions_contain_timeout() {
        assert!(ToolsPreset::Strict.description().contains("30s"));
        assert!(ToolsPreset::Balanced.description().contains("60s"));
        assert!(ToolsPreset::Permissive.description().contains("120s"));
    }

    #[test]
    fn test_preset_descriptions_mention_workspace() {
        assert!(ToolsPreset::Strict.description().contains("Workspace"));
        assert!(ToolsPreset::Balanced.description().contains("Workspace"));
        assert!(ToolsPreset::Permissive
            .description()
            .contains("No workspace"));
    }

    // ========================================================================
    // Brave API config tests
    // ========================================================================

    #[test]
    fn test_brave_api_key_default_empty() {
        let config = Config::default();
        assert!(config.tools.web.brave_api_key.is_empty());
    }

    #[test]
    fn test_brave_api_key_set_and_check() {
        let mut config = Config::default();
        config.tools.web.brave_api_key =
            config::schema::Secret::new("BSA-test-key-12345".to_string());
        assert!(!config.tools.web.brave_api_key.is_empty());
        assert_eq!(
            config.tools.web.brave_api_key.expose(),
            "BSA-test-key-12345"
        );
    }

    // ========================================================================
    // Timeout config tests
    // ========================================================================

    #[test]
    fn test_default_timeout() {
        let config = Config::default();
        assert_eq!(config.tools.exec.timeout, 60);
    }

    #[test]
    fn test_timeout_modification() {
        let mut config = Config::default();
        config.tools.exec.timeout = 120;
        assert_eq!(config.tools.exec.timeout, 120);

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.tools.exec.timeout, 120);
    }

    // ========================================================================
    // Allowlist config tests
    // ========================================================================

    #[test]
    fn test_allowlist_default_empty() {
        let config = Config::default();
        assert!(config.tools.exec.allowed_commands.is_empty());
    }

    #[test]
    fn test_allowlist_serialization() {
        let mut config = Config::default();
        config.tools.exec.allowed_commands =
            vec!["ls".to_string(), "cat".to_string(), "grep".to_string()];

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("allowedCommands"));

        let loaded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.tools.exec.allowed_commands.len(), 3);
        assert!(loaded
            .tools
            .exec
            .allowed_commands
            .contains(&"ls".to_string()));
    }

    // ========================================================================
    // Setting info tests
    // ========================================================================

    #[test]
    fn test_tool_settings_count() {
        assert_eq!(TOOL_SETTINGS.len(), 5);
    }

    #[test]
    fn test_tool_settings_keys_unique() {
        let keys: Vec<&str> = TOOL_SETTINGS.iter().map(|s| s.key).collect();
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(keys.len(), unique.len());
    }

    #[test]
    fn test_get_setting_status_preset() {
        let config = Config::default();
        assert_eq!(get_setting_status(&config, "preset"), "Balanced");
    }

    #[test]
    fn test_get_setting_status_workspace() {
        let mut config = Config::default();
        config.tools.restrict_to_workspace = true;
        assert_eq!(get_setting_status(&config, "workspace"), "enabled");

        config.tools.restrict_to_workspace = false;
        assert_eq!(get_setting_status(&config, "workspace"), "disabled");
    }

    #[test]
    fn test_get_setting_status_allowlist_empty() {
        let config = Config::default();
        assert_eq!(get_setting_status(&config, "allowlist"), "deny-list mode");
    }

    #[test]
    fn test_get_setting_status_allowlist_with_commands() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict);
        assert_eq!(get_setting_status(&config, "allowlist"), "10 commands");
    }

    #[test]
    fn test_get_setting_status_timeout() {
        let config = Config::default();
        assert_eq!(get_setting_status(&config, "timeout"), "60s");
    }

    #[test]
    fn test_get_setting_status_brave_not_configured() {
        let config = Config::default();
        assert_eq!(get_setting_status(&config, "brave_api"), "not configured");
    }

    #[test]
    fn test_get_setting_status_brave_configured() {
        let mut config = Config::default();
        config.tools.web.brave_api_key =
            config::schema::Secret::new("BSA-test-key-12345".to_string());
        let status = get_setting_status(&config, "brave_api");
        assert!(status.starts_with("configured"));
    }

    // ========================================================================
    // Sub-action generation tests
    // ========================================================================

    #[test]
    fn test_preset_sub_actions_exclude_current() {
        let config = Config::default(); // Balanced
        let actions = get_sub_actions(&config, 0); // preset
                                                   // Should have Strict, Permissive, Close (not Balanced)
        assert_eq!(actions.len(), 3);
        assert!(actions.contains(&ToolSubAction::SwitchPreset(ToolsPreset::Strict)));
        assert!(actions.contains(&ToolSubAction::SwitchPreset(ToolsPreset::Permissive)));
        assert!(actions.contains(&ToolSubAction::Close));
    }

    #[test]
    fn test_workspace_sub_actions() {
        let config = Config::default();
        let actions = get_sub_actions(&config, 1); // workspace
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&ToolSubAction::ToggleWorkspace));
        assert!(actions.contains(&ToolSubAction::Close));
    }

    #[test]
    fn test_allowlist_sub_actions_deny_list_mode() {
        let config = Config::default(); // empty allowlist = deny-list
        let actions = get_sub_actions(&config, 2); // allowlist
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&ToolSubAction::SwitchAllowlistMode));
        assert!(actions.contains(&ToolSubAction::Close));
    }

    #[test]
    fn test_allowlist_sub_actions_allowlist_mode() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict); // has commands
        let actions = get_sub_actions(&config, 2); // allowlist
        assert_eq!(actions.len(), 3);
        assert!(actions.contains(&ToolSubAction::EditAllowlist));
        assert!(actions.contains(&ToolSubAction::SwitchAllowlistMode));
        assert!(actions.contains(&ToolSubAction::Close));
    }

    #[test]
    fn test_brave_sub_actions_not_configured() {
        let config = Config::default();
        let actions = get_sub_actions(&config, 3); // brave_api
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&ToolSubAction::ConfigureBraveKey));
        assert!(actions.contains(&ToolSubAction::Close));
    }

    #[test]
    fn test_brave_sub_actions_configured() {
        let mut config = Config::default();
        config.tools.web.brave_api_key = config::schema::Secret::new("BSA-test-key".to_string());
        let actions = get_sub_actions(&config, 3); // brave_api
        assert_eq!(actions.len(), 3);
        assert!(actions.contains(&ToolSubAction::EditBraveKey));
        assert!(actions.contains(&ToolSubAction::RemoveBraveKey));
        assert!(actions.contains(&ToolSubAction::Close));
    }

    #[test]
    fn test_timeout_sub_actions() {
        let config = Config::default();
        let actions = get_sub_actions(&config, 4); // timeout
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&ToolSubAction::ChangeTimeout));
        assert!(actions.contains(&ToolSubAction::Close));
    }

    // ========================================================================
    // Menu state tests
    // ========================================================================

    #[test]
    fn test_menu_state_initial() {
        let menu = ToolMenuState::new();
        assert_eq!(menu.cursor, 0);
        assert!(menu.expanded.is_none());
        assert_eq!(menu.sub_cursor, 0);
        assert!(!menu.in_sub_menu);
    }

    #[test]
    fn test_menu_state_total_items() {
        let menu = ToolMenuState::new();
        assert_eq!(menu.total_main_items(), 6); // 5 settings + Done
    }

    #[test]
    fn test_menu_state_is_on_done() {
        let mut menu = ToolMenuState::new();
        assert!(!menu.is_on_done());
        menu.cursor = TOOL_SETTINGS.len();
        assert!(menu.is_on_done());
    }

    // ========================================================================
    // Icon tests
    // ========================================================================

    #[test]
    fn test_setting_icon_preset_always_star() {
        let config = Config::default();
        let (icon, _) = get_setting_icon(&config, "preset");
        assert_eq!(icon, "★");
    }

    #[test]
    fn test_setting_icon_workspace_enabled() {
        let mut config = Config::default();
        config.tools.restrict_to_workspace = true;
        let (icon, _) = get_setting_icon(&config, "workspace");
        assert_eq!(icon, "✓");
    }

    #[test]
    fn test_setting_icon_workspace_disabled() {
        let mut config = Config::default();
        config.tools.restrict_to_workspace = false;
        let (icon, _) = get_setting_icon(&config, "workspace");
        assert_eq!(icon, "○");
    }

    #[test]
    fn test_setting_icon_brave_not_configured() {
        let config = Config::default();
        let (icon, _) = get_setting_icon(&config, "brave_api");
        assert_eq!(icon, "○");
    }

    #[test]
    fn test_setting_icon_brave_configured() {
        let mut config = Config::default();
        config.tools.web.brave_api_key = config::schema::Secret::new("BSA-test".to_string());
        let (icon, _) = get_setting_icon(&config, "brave_api");
        assert_eq!(icon, "✓");
    }
}
