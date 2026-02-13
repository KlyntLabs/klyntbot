//! Tools configuration wizard step.
//!
//! Provides interactive setup for:
//! - Security preset profiles (strict, balanced, permissive)
//! - Workspace restriction toggle
//! - Shell command allowlist editing
//! - Brave Search API key
//! - Execution timeout

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

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

/// Run the tools configuration wizard step.
/// Returns true if tools were configured, false if skipped.
pub fn configure_tools(config: &mut Config) -> Result<bool> {
    let wants_config = prompt_yes_no("Would you like to configure tool permissions?", false)?;
    if !wants_config {
        println!(
            "\n  {} Using default tools configuration (balanced)",
            status_success()
        );
        return Ok(false);
    }

    println!();

    // Step 1: Choose preset profile
    let preset = select_preset()?;
    apply_preset(config, preset);
    println!(
        "\n  {} Applied {} preset",
        status_success(),
        match preset {
            ToolsPreset::Strict => "strict",
            ToolsPreset::Balanced => "balanced",
            ToolsPreset::Permissive => "permissive",
        }
    );

    // Step 2: Fine-tune workspace restriction
    let restrict = prompt_yes_no(
        "\nRestrict file/exec tools to workspace directory?",
        config.tools.restrict_to_workspace,
    )?;
    config.tools.restrict_to_workspace = restrict;
    if restrict {
        println!(
            "  {} Workspace restriction {}",
            status_success(),
            colorize("enabled", SUCCESS)
        );
    } else {
        println!(
            "  {} Workspace restriction {}",
            status_warning(),
            colorize("disabled", WARNING)
        );
    }

    // Step 3: Shell command allowlist
    configure_allowlist(config)?;

    // Step 4: Brave Search API key (optional)
    configure_brave_api(config)?;

    // Step 5: Execution timeout
    configure_timeout(config)?;

    println!(
        "\n  {} Tools configuration complete",
        status_success()
    );

    Ok(true)
}

/// Select a security preset profile
fn select_preset() -> Result<ToolsPreset> {
    println!("Select a security preset:\n");

    let presets = [
        ToolsPreset::Strict,
        ToolsPreset::Balanced,
        ToolsPreset::Permissive,
    ];

    for (idx, preset) in presets.iter().enumerate() {
        let label = match preset {
            ToolsPreset::Strict => "Strict",
            ToolsPreset::Balanced => "Balanced",
            ToolsPreset::Permissive => "Permissive",
        };
        println!(
            "  {}. {} - {}",
            colorize(&(idx + 1).to_string(), BOLD),
            label,
            colorize(preset.description(), DIM)
        );
    }
    println!();

    loop {
        print!("Preset [2]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(ToolsPreset::Balanced);
        }

        match input {
            "1" => return Ok(ToolsPreset::Strict),
            "2" => return Ok(ToolsPreset::Balanced),
            "3" => return Ok(ToolsPreset::Permissive),
            _ => {
                println!(
                    "{}",
                    colorize("Please enter 1, 2, or 3", ERROR)
                );
            }
        }
    }
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
    if config.tools.exec.allowed_commands.is_empty() {
        let wants_allowlist = prompt_yes_no(
            "\nAdd a shell command allowlist? (empty = deny-list mode)",
            false,
        )?;
        if !wants_allowlist {
            println!(
                "  {} Using deny-list mode {}",
                status_success(),
                colorize("(blocks dangerous patterns only)", DIM)
            );
            return Ok(());
        }
    } else {
        println!(
            "\n  Current allowlist: {}",
            colorize(&config.tools.exec.allowed_commands.join(", "), DIM)
        );
        let modify = prompt_yes_no("Modify the command allowlist?", true)?;
        if !modify {
            return Ok(());
        }
    }

    println!(
        "\n{}",
        colorize(
            "  Enter commands one per line (empty line to finish):",
            DIM
        )
    );

    let mut commands: Vec<String> = config.tools.exec.allowed_commands.clone();

    // Show current commands for editing
    if !commands.is_empty() {
        println!("  Current: {}", commands.join(", "));
        let clear = prompt_yes_no("  Clear existing and start fresh?", false)?;
        if clear {
            commands.clear();
        }
    }

    loop {
        print!("  {} ", colorize("+", SUCCESS));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            break;
        }

        if commands.contains(&input) {
            println!(
                "    {}",
                colorize("(already in list)", DIM)
            );
            continue;
        }

        commands.push(input);
    }

    if commands.is_empty() {
        println!(
            "  {} Allowlist cleared, using deny-list mode",
            status_success()
        );
    } else {
        println!(
            "  {} Allowlist set: {}",
            status_success(),
            commands.join(", ")
        );
    }

    config.tools.exec.allowed_commands = commands;
    Ok(())
}

/// Configure Brave Search API key
fn configure_brave_api(config: &mut Config) -> Result<()> {
    let has_key = !config.tools.web.brave_api_key.is_empty();
    let prompt = if has_key {
        "Update Brave Search API key?"
    } else {
        "Configure Brave Search API key? (enables web_search tool)"
    };

    let wants_api = prompt_yes_no(&format!("\n{}", prompt), false)?;
    if !wants_api {
        if has_key {
            println!("  {} Brave API key unchanged", status_success());
        } else {
            println!(
                "  {} Web search disabled {}",
                status_disabled(),
                colorize("(no API key)", DIM)
            );
        }
        return Ok(());
    }

    println!(
        "  Get yours at: {}",
        colorize("https://brave.com/search/api/", UNDERLINE)
    );

    loop {
        print!("  API Key: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            println!("  {} Skipped", status_disabled());
            return Ok(());
        }

        if input.len() < 10 {
            println!(
                "  {}",
                colorize("API key seems too short, try again", WARNING)
            );
            continue;
        }

        config.tools.web.brave_api_key = config::schema::Secret::new(input);
        println!("  {} Brave Search API key configured", status_success());
        return Ok(());
    }
}

/// Configure execution timeout
fn configure_timeout(config: &mut Config) -> Result<()> {
    println!(
        "\n  Current timeout: {}s",
        colorize(&config.tools.exec.timeout.to_string(), BOLD)
    );

    let modify = prompt_yes_no("Change execution timeout?", false)?;
    if !modify {
        return Ok(());
    }

    loop {
        print!(
            "  Timeout in seconds [{}]: ",
            config.tools.exec.timeout
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(());
        }

        match input.parse::<u64>() {
            Ok(secs) if (5..=600).contains(&secs) => {
                config.tools.exec.timeout = secs;
                println!(
                    "  {} Timeout set to {}s",
                    status_success(),
                    secs
                );
                return Ok(());
            }
            Ok(_) => {
                println!(
                    "  {}",
                    colorize("Timeout must be between 5 and 600 seconds", ERROR)
                );
            }
            Err(_) => {
                println!(
                    "  {}",
                    colorize("Please enter a number", ERROR)
                );
            }
        }
    }
}

/// Prompt for yes/no input (local copy matching wizard pattern)
fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", prompt, default_str);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return Ok(default);
    }

    Ok(input == "y" || input == "yes")
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
        assert!(config.tools.exec.allowed_commands.contains(&"ls".to_string()));
        assert!(config.tools.exec.allowed_commands.contains(&"cat".to_string()));
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
        // Ensure all presets have non-empty descriptions
        assert!(!ToolsPreset::Strict.description().is_empty());
        assert!(!ToolsPreset::Balanced.description().is_empty());
        assert!(!ToolsPreset::Permissive.description().is_empty());
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
        assert_eq!(config.tools.exec.allowed_commands.len(), safe_commands.len());

        for cmd in safe_commands {
            assert!(
                config.tools.exec.allowed_commands.contains(&cmd.to_string()),
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
                !config.tools.exec.allowed_commands.contains(&cmd.to_string()),
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

        // Start strict
        apply_preset(&mut config, ToolsPreset::Strict);
        assert!(config.tools.restrict_to_workspace);
        assert!(!config.tools.exec.allowed_commands.is_empty());

        // Switch to permissive
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
        assert!(loaded.tools.exec.allowed_commands.contains(&"ls".to_string()));
    }

    #[test]
    fn test_preset_descriptions_contain_timeout() {
        // Each preset description should mention its timeout
        assert!(ToolsPreset::Strict.description().contains("30s"));
        assert!(ToolsPreset::Balanced.description().contains("60s"));
        assert!(ToolsPreset::Permissive.description().contains("120s"));
    }

    #[test]
    fn test_preset_descriptions_mention_workspace() {
        assert!(ToolsPreset::Strict.description().contains("Workspace"));
        assert!(ToolsPreset::Balanced.description().contains("Workspace"));
        // Permissive does NOT restrict to workspace
        assert!(ToolsPreset::Permissive.description().contains("No workspace"));
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

        // Verify serialization preserves it
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
        assert!(loaded.tools.exec.allowed_commands.contains(&"ls".to_string()));
    }
}
