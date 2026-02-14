//! Tools configuration wizard step.
//!
//! Provides interactive setup for:
//! - Security preset profiles (strict, balanced, permissive)
//! - Workspace restriction toggle
//! - Shell command allowlist editing
//! - Brave Search API key
//! - Execution timeout
//!
//! On reconfiguration, all fields show current values as defaults.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

use super::prompts::{self, SelectOption};

/// Security preset profile for tools configuration
#[derive(Debug, Clone, Copy)]
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
}

/// Detect the current preset from config values.
fn detect_preset(config: &Config) -> usize {
    if config.tools.restrict_to_workspace
        && !config.tools.exec.allowed_commands.is_empty()
        && config.tools.exec.timeout <= 30
    {
        0 // Strict
    } else if !config.tools.restrict_to_workspace && config.tools.exec.timeout >= 120 {
        2 // Permissive
    } else {
        1 // Balanced
    }
}

/// Run the tools configuration wizard step.
/// Returns true if tools were configured, false if skipped.
pub fn configure_tools(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    // Show current status
    let presets = [
        ToolsPreset::Strict,
        ToolsPreset::Balanced,
        ToolsPreset::Permissive,
    ];
    let current_preset_idx = detect_preset(config);
    let preset_name = match current_preset_idx {
        0 => "Strict",
        2 => "Permissive",
        _ => "Balanced",
    };
    println!(
        "{} Current: {} preset, timeout {}s, workspace restriction {}",
        colorize(chars.vertical, BRAND),
        colorize(preset_name, BOLD),
        colorize(&config.tools.exec.timeout.to_string(), DIM),
        if config.tools.restrict_to_workspace {
            colorize("on", SUCCESS)
        } else {
            colorize("off", WARNING)
        }
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Step 1: Choose preset profile (pre-select current)
    let options: Vec<SelectOption<'_>> = presets
        .iter()
        .map(|p| {
            let label = match p {
                ToolsPreset::Strict => "Strict",
                ToolsPreset::Balanced => "Balanced",
                ToolsPreset::Permissive => "Permissive",
            };
            SelectOption {
                label,
                description: p.description(),
            }
        })
        .collect();

    let idx = prompts::prompt_select("Select a security preset", &options, current_preset_idx)?;
    let preset = presets[idx];
    apply_preset(config, preset);
    println!(
        "{} {} Applied {} preset",
        colorize(chars.vertical, BRAND),
        status_success(),
        match preset {
            ToolsPreset::Strict => "strict",
            ToolsPreset::Balanced => "balanced",
            ToolsPreset::Permissive => "permissive",
        }
    );

    // Step 2: Fine-tune workspace restriction (default from preset)
    println!("{}", colorize(chars.vertical, BRAND));
    let restrict = prompts::prompt_yes_no(
        "Restrict file/exec tools to workspace directory?",
        config.tools.restrict_to_workspace,
    )?;
    config.tools.restrict_to_workspace = restrict;
    if restrict {
        println!(
            "{} {} Workspace restriction {}",
            colorize(chars.vertical, BRAND),
            status_success(),
            colorize("enabled", SUCCESS)
        );
    } else {
        println!(
            "{} {} Workspace restriction {}",
            colorize(chars.vertical, BRAND),
            status_warning(),
            colorize("disabled", WARNING)
        );
    }

    // Step 3: Shell command allowlist
    configure_allowlist(config)?;

    // Step 4: Brave Search API key (edit-in-place)
    configure_brave_api(config)?;

    // Step 5: Execution timeout (edit-in-place)
    configure_timeout(config)?;

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Tools configuration complete",
        colorize(chars.vertical, BRAND),
        status_success()
    );

    Ok(true)
}

/// Apply a preset profile to the config
fn apply_preset(config: &mut Config, preset: ToolsPreset) {
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
            config.tools.exec.allowed_commands = Vec::new(); // deny-list mode
        }
        ToolsPreset::Permissive => {
            config.tools.restrict_to_workspace = false;
            config.tools.exec.timeout = 120;
            config.tools.exec.allowed_commands = Vec::new(); // deny-list mode
        }
    }
}

/// Configure the shell command allowlist
fn configure_allowlist(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    if config.tools.exec.allowed_commands.is_empty() {
        println!("{}", colorize(chars.vertical, BRAND));
        let wants_allowlist = prompts::prompt_yes_no(
            "Add a shell command allowlist? (empty = deny-list mode)",
            false,
        )?;
        if !wants_allowlist {
            println!(
                "{} {} Using deny-list mode {}",
                colorize(chars.vertical, BRAND),
                status_success(),
                colorize("(blocks dangerous patterns only)", DIM)
            );
            return Ok(());
        }
    } else {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} Current allowlist: {}",
            colorize(chars.vertical, BRAND),
            colorize(&config.tools.exec.allowed_commands.join(", "), DIM)
        );
        let modify = prompts::prompt_yes_no("Modify the command allowlist?", false)?;
        if !modify {
            return Ok(());
        }
    }

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize(
            "Enter commands one per line (empty line to finish):",
            DIM
        ))
    );

    let mut commands: Vec<String> = config.tools.exec.allowed_commands.clone();

    // Show current commands for editing
    if !commands.is_empty() {
        println!(
            "{} Current: {}",
            colorize(chars.vertical, BRAND),
            commands.join(", ")
        );
        let clear = prompts::prompt_yes_no("Clear existing and start fresh?", false)?;
        if clear {
            commands.clear();
        }
    }

    loop {
        print!(
            "{} {} ",
            colorize(chars.vertical, BRAND),
            colorize("+", SUCCESS)
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            break;
        }

        if commands.contains(&input) {
            println!(
                "{}   {}",
                colorize(chars.vertical, BRAND),
                colorize("(already in list)", DIM)
            );
            continue;
        }

        commands.push(input);
    }

    if commands.is_empty() {
        println!(
            "{} {} Allowlist cleared, using deny-list mode",
            colorize(chars.vertical, BRAND),
            status_success()
        );
    } else {
        println!(
            "{} {} Allowlist set: {}",
            colorize(chars.vertical, BRAND),
            status_success(),
            commands.join(", ")
        );
    }

    config.tools.exec.allowed_commands = commands;
    Ok(())
}

/// Configure Brave Search API key (edit-in-place with masked preview)
fn configure_brave_api(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();
    println!("{}", colorize(chars.vertical, BRAND));

    let existing_key = config.tools.web.brave_api_key.expose().clone();

    if existing_key.is_empty() {
        // No existing key - ask if they want to add one
        let wants_api = prompts::prompt_yes_no(
            "Configure Brave Search API key? (enables web_search tool)",
            false,
        )?;
        if !wants_api {
            println!(
                "{} {} Web search disabled {}",
                colorize(chars.vertical, BRAND),
                status_disabled(),
                colorize("(no API key)", DIM)
            );
            return Ok(());
        }

        println!(
            "{} Get yours at: {}",
            colorize(chars.vertical, BRAND),
            colorize("https://brave.com/search/api/", UNDERLINE)
        );

        let key = prompts::prompt_secret("Brave API Key", 10)?;
        config.tools.web.brave_api_key = config::schema::Secret::new(key);
        println!(
            "{} {} Brave Search API key configured",
            colorize(chars.vertical, BRAND),
            status_success()
        );
    } else {
        // Has existing key - edit-in-place
        match prompts::prompt_secret_with_existing("Brave API Key", &existing_key, 10)? {
            Some(new_key) => {
                config.tools.web.brave_api_key = config::schema::Secret::new(new_key);
                println!(
                    "{} {} Brave Search API key updated",
                    colorize(chars.vertical, BRAND),
                    status_success()
                );
            }
            None => {
                println!(
                    "{} {} Brave API key unchanged",
                    colorize(chars.vertical, BRAND),
                    status_success()
                );
            }
        }
    }

    Ok(())
}

/// Configure execution timeout (edit-in-place with current as default)
fn configure_timeout(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    println!("{}", colorize(chars.vertical, BRAND));

    let current = config.tools.exec.timeout.to_string();
    let input = prompts::prompt_text("Execution timeout (seconds)", Some(&current), true)?;

    match input.parse::<u64>() {
        Ok(secs) if (5..=600).contains(&secs) => {
            config.tools.exec.timeout = secs;
            println!(
                "{} {} Timeout set to {}s",
                colorize(chars.vertical, BRAND),
                status_success(),
                secs
            );
        }
        Ok(_) => {
            println!(
                "{} {}",
                colorize(chars.vertical, BRAND),
                colorize(
                    "Timeout must be between 5 and 600 seconds, keeping current",
                    WARNING
                )
            );
        }
        Err(_) => {
            println!(
                "{} {}",
                colorize(chars.vertical, BRAND),
                colorize("Invalid number, keeping current timeout", WARNING)
            );
        }
    }

    Ok(())
}

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
        assert_eq!(detect_preset(&config), 1); // Balanced
    }

    #[test]
    fn test_detect_preset_strict() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Strict);
        assert_eq!(detect_preset(&config), 0); // Strict
    }

    #[test]
    fn test_detect_preset_permissive() {
        let mut config = Config::default();
        apply_preset(&mut config, ToolsPreset::Permissive);
        assert_eq!(detect_preset(&config), 2); // Permissive
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
}
