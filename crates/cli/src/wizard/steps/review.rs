//! Review & Confirm wizard step.
//!
//! Displays a summary of all configured settings before saving,
//! with options to go back and edit or save and finish.

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use common::utils::terminal::*;
use crossterm::{event::KeyCode, terminal};

use crate::wizard::framework::{StepResult, WizardModule, WizardState};
use crate::wizard::prompts;
use crate::wizard::ui::{erase_lines, read_key};

pub struct ReviewModule;

impl WizardModule for ReviewModule {
    fn name(&self) -> &str {
        "Review & Confirm"
    }

    fn description(&self) -> &str {
        "Review your configuration before saving"
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let chars = BoxChars::get();
        let prefix = format!("{} ", colorize(chars.vertical, BRAND));

        println!(
            "{}",
            draw_step_line(&colorize("Configuration Summary", BOLD))
        );
        println!("{}", colorize(chars.vertical, BRAND));

        // Database
        if let Some(ref url) = state.config.database_url {
            println!(
                "{} {} Database: {}",
                prefix.trim_end(),
                colorize("✓", SUCCESS),
                colorize(&format!("PostgreSQL ({})", mask_database_url(url)), DIM),
            );
        } else {
            println!(
                "{} {} Database: {}",
                prefix.trim_end(),
                colorize("○", DIM),
                colorize("JSONL files (default)", DIM),
            );
        }

        // Provider
        let active = state.config.active_provider_name();
        if active != "none" {
            let model = &state.config.agents.defaults.model;
            println!(
                "{} {} Provider: {} ({})",
                prefix.trim_end(),
                colorize("★", HIGHLIGHT),
                colorize(&capitalize(active), BOLD),
                colorize(model, DIM)
            );
        } else {
            println!(
                "{} {} Provider: {}",
                prefix.trim_end(),
                colorize("○", DIM),
                colorize("Not configured", DIM)
            );
        }

        // Channels
        let enabled_channels = get_enabled_channels(&state.config);
        if enabled_channels.is_empty() {
            println!(
                "{} {} Channels: {}",
                prefix.trim_end(),
                colorize("○", DIM),
                colorize("None configured", DIM)
            );
        } else {
            println!(
                "{} {} Channels: {} ({})",
                prefix.trim_end(),
                colorize("✓", SUCCESS),
                enabled_channels.join(", "),
                colorize(&format!("{} enabled", enabled_channels.len()), DIM)
            );
        }

        // Tools
        let preset = crate::wizard::tools::detect_preset(&state.config);
        println!(
            "{} {} Tools: {} preset, {}s timeout",
            prefix.trim_end(),
            colorize("✓", SUCCESS),
            colorize(preset.name(), BOLD),
            state.config.tools.exec.timeout
        );

        // Workspace
        let workspace = state.config.workspace_path();
        println!(
            "{} {} Workspace: {}",
            prefix.trim_end(),
            colorize("✓", SUCCESS),
            colorize(&shorten_path(&workspace), DIM)
        );

        // Calendar
        if state.config.calendar.is_any_enabled() {
            let enabled = state.config.calendar.enabled_providers();
            let names: Vec<&str> = enabled.iter().map(|p| p.display_name()).collect();
            println!(
                "{} {} Calendar: {}",
                prefix.trim_end(),
                colorize("✓", SUCCESS),
                names.join(", ")
            );
        } else {
            println!(
                "{} {} Calendar: {}",
                prefix.trim_end(),
                colorize("○", DIM),
                colorize("Not configured", DIM)
            );
        }

        println!("{}", colorize(chars.vertical, BRAND));

        // Interactive choice: Back to Edit or Save & Finish
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            run_review_menu()
        } else {
            run_review_fallback()
        }
    }
}

/// Interactive two-option menu: "← Back to Edit" and "Save & Finish".
fn run_review_menu() -> Result<StepResult> {
    let chars = BoxChars::get();
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let mut out = io::stdout();
    let mut cursor: usize = 1; // default to "Save & Finish"

    terminal::enable_raw_mode()?;
    let mut lines = render_review_options(&mut out, &prefix, cursor, chars)?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if cursor > 0 {
                    cursor -= 1;
                    lines = rerender_review(&mut out, &prefix, cursor, chars, lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if cursor < 1 {
                    cursor += 1;
                    lines = rerender_review(&mut out, &prefix, cursor, chars, lines)?;
                }
            }
            KeyCode::Enter => {
                terminal::disable_raw_mode()?;
                erase_lines(lines + 1)?; // +1 for hint
                return if cursor == 0 {
                    Ok(StepResult::Back)
                } else {
                    Ok(StepResult::Next)
                };
            }
            KeyCode::Esc => {
                terminal::disable_raw_mode()?;
                erase_lines(lines + 1)?;
                return Ok(StepResult::Back);
            }
            _ => {}
        }
    }
}

/// Render the two review options. Returns lines rendered.
fn render_review_options(
    out: &mut impl Write,
    prefix: &str,
    cursor: usize,
    chars: &BoxChars,
) -> Result<usize> {
    let mut lines = 0;

    // Separator
    write!(
        out,
        "{}  {}\r\n",
        colorize(chars.vertical, BRAND),
        colorize("──────────────────────────", DIM)
    )?;
    lines += 1;

    // Option 0: Back to Edit
    let back_pointer = if cursor == 0 {
        colorize("❯", BRAND)
    } else {
        " ".to_string()
    };
    let back_label = if cursor == 0 {
        colorize("← Back to Edit", BOLD)
    } else {
        "← Back to Edit".to_string()
    };
    write!(
        out,
        "{}{} {} {}\r\n",
        prefix,
        back_pointer,
        colorize("◁", DIM),
        back_label
    )?;
    lines += 1;

    // Option 1: Save & Finish
    let save_pointer = if cursor == 1 {
        colorize("❯", BRAND)
    } else {
        " ".to_string()
    };
    let save_label = if cursor == 1 {
        colorize("Save & Finish", BOLD)
    } else {
        "Save & Finish".to_string()
    };
    write!(
        out,
        "{}{} {} {}\r\n",
        prefix,
        save_pointer,
        colorize("●", BRAND),
        save_label
    )?;
    lines += 1;

    // Hint
    write!(
        out,
        "{}{}\r\n",
        prefix,
        colorize("↑/↓ navigate · Enter select · Esc back", DIM)
    )?;

    out.flush()?;
    Ok(lines)
}

/// Erase and re-render review options. Returns new line count.
fn rerender_review(
    out: &mut impl Write,
    prefix: &str,
    cursor: usize,
    chars: &BoxChars,
    prev_lines: usize,
) -> Result<usize> {
    let total = prev_lines + 1; // +1 for hint
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    render_review_options(out, prefix, cursor, chars)
}

/// Non-TTY fallback for the review step.
fn run_review_fallback() -> Result<StepResult> {
    let options = vec![
        prompts::SelectOption {
            label: "Save & Finish",
            description: "Save configuration and exit",
        },
        prompts::SelectOption {
            label: "← Back to Edit",
            description: "Return to previous step to make changes",
        },
    ];

    let idx = prompts::prompt_select("What would you like to do?", &options, 0)?;

    if idx == 1 {
        Ok(StepResult::Back)
    } else {
        Ok(StepResult::Next)
    }
}

/// Get list of enabled channel names.
fn get_enabled_channels(config: &config::Config) -> Vec<&'static str> {
    let mut channels = Vec::new();
    if config.channels.telegram.enabled {
        channels.push("Telegram");
    }
    if config.channels.discord.enabled {
        channels.push("Discord");
    }
    if config.channels.slack.enabled {
        channels.push("Slack");
    }
    if config.channels.whatsapp.enabled {
        channels.push("WhatsApp");
    }
    if config.channels.email.enabled {
        channels.push("Email");
    }
    if config.channels.qq.enabled {
        channels.push("QQ");
    }
    channels
}

/// Shorten a path for display (use ~ for home dir).
fn shorten_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

/// Mask credentials in a database URL for display.
fn mask_database_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let scheme = &url[..scheme_end + 3];
            let after_at = &url[at_pos..];
            return format!("{}***{}", scheme, after_at);
        }
    }
    url.to_string()
}

/// Capitalize the first letter of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_module_metadata() {
        let module = ReviewModule;
        assert_eq!(module.name(), "Review & Confirm");
        assert!(module.is_required());
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("anthropic"), "Anthropic");
        assert_eq!(capitalize("openAI"), "OpenAI");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_get_enabled_channels_empty() {
        let config = config::Config::default();
        assert!(get_enabled_channels(&config).is_empty());
    }

    #[test]
    fn test_get_enabled_channels_some() {
        let mut config = config::Config::default();
        config.channels.telegram.enabled = true;
        config.channels.discord.enabled = true;
        let channels = get_enabled_channels(&config);
        assert_eq!(channels.len(), 2);
        assert!(channels.contains(&"Telegram"));
        assert!(channels.contains(&"Discord"));
    }
}
