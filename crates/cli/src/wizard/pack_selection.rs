//! Phase 2: Pack selection checklist.
//!
//! Shows all packs grouped by tier, lets user toggle with spacebar, confirms
//! with enter.  Also applies config mutations for each enabled pack.

use anyhow::Result;
use config::{Config, PackTier, PacksConfig};

use super::framework::{StepResult, WizardState};
use super::packs::registry::PackRegistry;

// ============================================================================
// Pack option (UI model)
// ============================================================================

/// A single row in the pack selection checklist.
#[derive(Debug)]
pub struct PackOption {
    /// Pack ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Tier label.
    pub tier: PackTier,
    /// Whether the option is currently checked.
    pub checked: bool,
    /// Whether the option is locked (Core packs cannot be unchecked).
    pub locked: bool,
}

// ============================================================================
// Build UI options from pack registry + current selection
// ============================================================================

/// Build the checklist options for all packs.
///
/// `currently_enabled` is the list of pack IDs that are currently enabled
/// (from `config.packs.enabled`).  Core packs are always locked + checked.
/// Recommended packs are pre-checked when they appear in `currently_enabled`.
/// Optional packs are unchecked unless in `currently_enabled`.
pub fn build_pack_options(currently_enabled: &[String]) -> Vec<PackOption> {
    PackRegistry::all()
        .into_iter()
        .map(|pack| {
            let locked = pack.tier == PackTier::Core;
            let checked = if locked {
                true
            } else {
                currently_enabled.iter().any(|e| e == pack.id)
            };

            PackOption {
                id: pack.id.to_string(),
                name: pack.name.to_string(),
                description: pack.description.to_string(),
                tier: pack.tier,
                checked,
                locked,
            }
        })
        .collect()
}

// ============================================================================
// Apply config mutations based on selected packs
// ============================================================================

/// Apply config mutations based on which packs are enabled.
///
/// Each pack maps to specific config section toggles.  When a pack is *not*
/// in the selection its corresponding sections are disabled.
pub fn apply_pack_config(config: &mut Config, enabled_packs: &[String]) {
    let has = |id: &str| enabled_packs.iter().any(|e| e == id);

    // --- task-management ---
    config.todo.enrichment.enabled = has("task-management");
    config.todo.search.enabled = has("task-management");

    // --- productivity ---
    config.todo.daily_planning.enabled = has("productivity");
    config.todo.notifications.daily_digest = has("productivity");

    // --- ai-intelligence ---
    config.conversation.embedding.enabled = has("ai-intelligence");
    config.conversation.search.enabled = has("ai-intelligence");
    config.learning.enabled = has("ai-intelligence");

    // --- developer-tools ---
    if has("developer-tools") {
        config.tools.restrict_to_workspace = true;
    }

    // --- finance ---
    config.finance.enabled = has("finance");

    // --- weather & skill-creator: skill-only, no config mutations ---

    // Update the packs config itself
    config.packs = PacksConfig {
        enabled: enabled_packs.to_vec(),
        enabled_skills: PackRegistry::skills_for_packs(enabled_packs),
    };
}

// ============================================================================
// Run pack selection (interactive)
// ============================================================================

/// Run the Phase 2 pack selection checklist.
///
/// Renders packs grouped by tier, lets user toggle with keyboard, applies
/// config mutations on confirmation.  Returns `StepResult::Next` on success
/// or `StepResult::Back` / `StepResult::Cancel` on navigation.
pub fn run_pack_selection(state: &mut WizardState) -> Result<StepResult> {
    use common::utils::terminal::*;

    println!(
        "  {} Select which feature packs to enable.\n",
        colorize("●", BRAND)
    );
    println!(
        "  {}  Use {} / {} to navigate, {} to toggle, {} to confirm.\n",
        colorize("Tip:", DIM),
        colorize("↑↓", BOLD),
        colorize("j/k", BOLD),
        colorize("Space", BOLD),
        colorize("Enter", BOLD),
    );

    let mut options = build_pack_options(&state.config.packs.enabled);

    // Interactive selection loop (crossterm raw mode)
    let mut cursor: usize = 0;

    // Find first non-locked option to start on
    if let Some(first_unlocked) = options.iter().position(|o| !o.locked) {
        cursor = first_unlocked;
    }

    crossterm::terminal::enable_raw_mode()?;
    let result = (|| -> Result<Option<Vec<String>>> {
        use crossterm::event::{self, Event, KeyCode, KeyModifiers};
        use std::io::Write;

        render_checklist(&options, cursor);

        loop {
            if let Event::Key(key) = event::read()? {
                // Ctrl+C cancels
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(None);
                }

                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        cursor = cursor.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if cursor + 1 < options.len() {
                            cursor += 1;
                        }
                    }
                    KeyCode::Char(' ') => {
                        if !options[cursor].locked {
                            options[cursor].checked = !options[cursor].checked;
                        }
                    }
                    KeyCode::Enter => {
                        let selected: Vec<String> = options
                            .iter()
                            .filter(|o| o.checked)
                            .map(|o| o.id.clone())
                            .collect();
                        // Clear the checklist rendering
                        erase_lines(options.len() + tier_header_count(&options));
                        std::io::stdout().flush()?;
                        return Ok(Some(selected));
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        erase_lines(options.len() + tier_header_count(&options));
                        std::io::stdout().flush()?;
                        return Ok(None);
                    }
                    _ => {}
                }

                // Re-render
                erase_lines(options.len() + tier_header_count(&options));
                render_checklist(&options, cursor);
            }
        }
    })();

    crossterm::terminal::disable_raw_mode()?;

    match result? {
        Some(selected) => {
            apply_pack_config(&mut state.config, &selected);

            // Print summary
            println!("  {} Enabled packs:", colorize("✓", SUCCESS));
            for id in &state.config.packs.enabled {
                if let Some(pack) = PackRegistry::get(id) {
                    println!("    {} {}", colorize("•", BRAND), pack.name);
                }
            }
            println!();

            Ok(StepResult::Next)
        }
        None => Ok(StepResult::Cancel),
    }
}

// ============================================================================
// Rendering helpers
// ============================================================================

/// Count how many tier headers will be rendered.
fn tier_header_count(options: &[PackOption]) -> usize {
    let mut count = 0;
    let mut last_tier: Option<PackTier> = None;
    for opt in options {
        if last_tier != Some(opt.tier) {
            count += 1;
            last_tier = Some(opt.tier);
        }
    }
    count
}

/// Render the checklist to stdout.
///
/// Uses explicit `\r\n` because this runs inside crossterm raw mode where
/// `\n` alone only moves the cursor down without returning to column 0.
fn render_checklist(options: &[PackOption], cursor: usize) {
    use common::utils::terminal::*;
    use std::io::Write;

    let mut stdout = std::io::stdout();
    let mut last_tier: Option<PackTier> = None;

    for (i, opt) in options.iter().enumerate() {
        // Print tier header when tier changes
        if last_tier != Some(opt.tier) {
            let label = match opt.tier {
                PackTier::Core => "Core (always enabled)",
                PackTier::Recommended => "Recommended",
                PackTier::Optional => "Optional",
            };
            let _ = write!(stdout, "  {}\r\n", colorize(label, BOLD));
            last_tier = Some(opt.tier);
        }

        let marker = if opt.locked {
            colorize("[■]", DIM)
        } else if opt.checked {
            colorize("[✓]", SUCCESS)
        } else {
            "[ ]".to_string()
        };

        let pointer = if i == cursor {
            colorize("▸ ", BRAND)
        } else {
            "  ".to_string()
        };

        let name_style = if opt.locked {
            colorize(&opt.name, DIM)
        } else {
            colorize(&opt.name, BOLD)
        };

        let _ = write!(
            stdout,
            "    {}{} {} {}\r\n",
            pointer,
            marker,
            name_style,
            colorize(&format!("— {}", opt.description), DIM)
        );
    }

    let _ = stdout.flush();
}

/// Erase N lines above the cursor (for re-rendering).
fn erase_lines(n: usize) {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    for _ in 0..n {
        // Move up one line and clear it
        let _ = write!(stdout, "\x1b[1A\x1b[2K");
    }
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pack_options_returns_all_packs() {
        let options = build_pack_options(&[]);
        assert_eq!(options.len(), 7);
    }

    #[test]
    fn test_build_pack_options_default_selection() {
        let defaults = PacksConfig::default().enabled;
        let options = build_pack_options(&defaults);

        // Core packs should be marked as locked
        let task = options.iter().find(|o| o.id == "task-management").unwrap();
        assert!(task.locked);
        assert!(task.checked);

        // Recommended should be pre-checked
        let prod = options.iter().find(|o| o.id == "productivity").unwrap();
        assert!(prod.checked);
        assert!(!prod.locked);

        // Optional should not be checked
        let weather = options.iter().find(|o| o.id == "weather").unwrap();
        assert!(!weather.checked);
        assert!(!weather.locked);
    }

    #[test]
    fn test_build_pack_options_custom_selection() {
        let enabled = vec!["task-management".to_string(), "weather".to_string()];
        let options = build_pack_options(&enabled);

        // Core is always locked+checked
        let task = options.iter().find(|o| o.id == "task-management").unwrap();
        assert!(task.locked);
        assert!(task.checked);

        // Weather is checked because it's in enabled list
        let weather = options.iter().find(|o| o.id == "weather").unwrap();
        assert!(weather.checked);

        // Productivity is not checked because it's not in enabled list
        let prod = options.iter().find(|o| o.id == "productivity").unwrap();
        assert!(!prod.checked);
    }

    #[test]
    fn test_apply_pack_config_enables_ai_intelligence() {
        let mut config = Config::default();
        let selection = vec!["task-management".to_string(), "ai-intelligence".to_string()];

        apply_pack_config(&mut config, &selection);

        // AI Intelligence enables conversation + learning
        assert!(config.conversation.embedding.enabled);
        assert!(config.conversation.search.enabled);
        assert!(config.learning.enabled);
    }

    #[test]
    fn test_apply_pack_config_disables_missing_packs() {
        let mut config = Config::default();
        // Only task-management — everything else should be disabled
        let selection = vec!["task-management".to_string()];

        apply_pack_config(&mut config, &selection);

        // Task management features enabled
        assert!(config.todo.enrichment.enabled);
        assert!(config.todo.search.enabled);

        // Productivity disabled
        assert!(!config.todo.daily_planning.enabled);
        assert!(!config.todo.notifications.daily_digest);

        // AI intelligence disabled
        assert!(!config.conversation.embedding.enabled);
        assert!(!config.conversation.search.enabled);
        assert!(!config.learning.enabled);

        // Finance disabled
        assert!(!config.finance.enabled);
    }

    #[test]
    fn test_apply_pack_config_updates_packs_config() {
        let mut config = Config::default();
        let selection = vec!["task-management".to_string(), "developer-tools".to_string()];

        apply_pack_config(&mut config, &selection);

        assert_eq!(config.packs.enabled, selection);
        assert!(config.packs.enabled_skills.contains(&"todo".to_string()));
        assert!(config.packs.enabled_skills.contains(&"github".to_string()));
        assert!(!config.packs.enabled_skills.contains(&"weather".to_string()));
    }

    #[test]
    fn test_apply_pack_config_developer_tools_restricts_workspace() {
        let mut config = Config::default();
        config.tools.restrict_to_workspace = false;

        let selection = vec!["task-management".to_string(), "developer-tools".to_string()];
        apply_pack_config(&mut config, &selection);

        assert!(config.tools.restrict_to_workspace);
    }

    #[test]
    fn test_tier_header_count() {
        let options = build_pack_options(&PacksConfig::default().enabled);
        // Should have 3 tier headers: Core, Recommended, Optional
        assert_eq!(tier_header_count(&options), 3);
    }
}
