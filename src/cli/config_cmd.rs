//! Config command handlers for configuration management

use anyhow::Result;
use crate::cli::ConfigCommands;

/// Handle config commands
pub async fn handle_config(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Show => {
            let config = crate::config::load()?;
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }
        ConfigCommands::Get { key } => {
            let config = crate::config::load()?;
            let json = serde_json::to_value(&config)?;

            // Navigate the dot-notation path
            match get_config_value(&json, &key) {
                Some(value) => {
                    println!("{}", serde_json::to_string_pretty(&value)?);
                }
                None => {
                    return Err(anyhow::anyhow!(
                        "Configuration key '{}' not found\n\nUse 'klyntbot config show' to see all available keys",
                        key
                    ));
                }
            }
        }
        ConfigCommands::Set { key, value } => {
            let config_path = crate::config::config_path()?;
            let content = std::fs::read_to_string(&config_path)?;
            let mut json: serde_json::Value = serde_json::from_str(&content)?;

            // Parse the value as JSON first, fall back to string if it fails
            let parsed_value = match serde_json::from_str::<serde_json::Value>(&value) {
                Ok(v) => v,
                Err(_) => serde_json::Value::String(value.clone()),
            };

            // Set the value at the dot-notation path
            if set_config_value(&mut json, &key, parsed_value)? {
                // Validate by deserializing
                let _: crate::config::Config = serde_json::from_value(json.clone())?;

                // Save back to file
                let content = serde_json::to_string_pretty(&json)?;
                std::fs::write(&config_path, content)?;

                println!("✓ Set {} = {}", key, value);
            } else {
                return Err(anyhow::anyhow!("Failed to set key '{}'", key));
            }
        }
        ConfigCommands::Edit => {
            let config_path = crate::config::config_path()?;

            // Get editor from environment
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
                if cfg!(target_os = "windows") {
                    "notepad".to_string()
                } else {
                    "vi".to_string()
                }
            });

            println!("Opening config in {}...", editor);

            // Open editor
            let status = std::process::Command::new(&editor)
                .arg(&config_path)
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!("Editor exited with error"));
            }

            // Validate the edited config
            match crate::config::load() {
                Ok(_) => {
                    println!("✓ Configuration is valid");
                }
                Err(e) => {
                    println!("✗ Configuration has errors: {}", e);
                    println!("\nFix errors and run 'klyntbot config validate' to check");
                    return Err(e.into());
                }
            }
        }
        ConfigCommands::Reset { force } => {
            let config_path = crate::config::config_path()?;

            if config_path.exists() && !force {
                return Err(anyhow::anyhow!(
                    "Configuration file already exists\n\nTo overwrite the existing configuration, use:\n  klyntbot config reset --force\n\nWarning: This will delete all your current settings"
                ));
            }

            // Create default config
            let default_config = crate::config::Config::default();
            crate::config::save(&default_config)?;

            println!("✓ Reset configuration to defaults");
            println!("Config: {}", config_path.display());
        }
        ConfigCommands::Validate => {
            let config = crate::config::load()?;
            println!("✓ Configuration is valid");
            println!("  Model: {}", config.agents.defaults.model);
            println!("  Workspace: {}", config.agents.defaults.workspace);
        }
    }
    Ok(())
}

/// Get a config value by dot-notation path
pub fn get_config_value<'a>(json: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = json;

    for part in parts {
        current = current.get(part)?;
    }

    Some(current)
}

/// Set a config value by dot-notation path
pub fn set_config_value(
    json: &mut serde_json::Value,
    key: &str,
    value: serde_json::Value,
) -> Result<bool> {
    let parts: Vec<&str> = key.split('.').collect();

    if parts.is_empty() {
        return Ok(false);
    }

    // Navigate to the parent object
    let mut current = json;
    for part in &parts[..parts.len() - 1] {
        current = current
            .get_mut(part)
            .ok_or_else(|| anyhow::anyhow!("Key path '{}' not found", part))?;
    }

    // Set the final value
    let last_key = parts[parts.len() - 1];
    if let Some(obj) = current.as_object_mut() {
        obj.insert(last_key.to_string(), value);
        Ok(true)
    } else {
        Err(anyhow::anyhow!("Cannot set value on non-object"))
    }
}
