//! Workspace setup wizard module.
//!
//! Creates the workspace directory structure and template files
//! (AGENTS.md, SOUL.md, USER.md, TOOLS.md, IDENTITY.md, MEMORY.md).

use anyhow::Result;
use common::utils::terminal::*;

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts;
use super::templates;

pub struct WorkspaceModule;

impl WizardModule for WorkspaceModule {
    fn name(&self) -> &str {
        "Workspace Setup"
    }

    fn description(&self) -> &str {
        "Create workspace directories and template files"
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let workspace = state.config.workspace_path();

        // Check if workspace already exists
        if workspace.exists() {
            println!(
                "  Workspace already exists at {}",
                colorize(&workspace.display().to_string(), DIM)
            );
            let recreate = prompts::prompt_yes_no(
                "  Re-create workspace files? (existing files will be preserved)",
                false,
            )?;
            if !recreate {
                println!("  {} Workspace unchanged", status_success());
                return Ok(StepResult::Next);
            }
        }

        // Allow custom workspace path
        let custom = prompts::prompt_yes_no(
            &format!(
                "Use default workspace path ({})? ",
                colorize(&workspace.display().to_string(), DIM)
            ),
            true,
        )?;

        if !custom {
            let new_path = prompts::prompt_text(
                "  Workspace path",
                Some(&workspace.display().to_string()),
                true,
            )?;
            state.config.agents.defaults.workspace = new_path;
        }

        let workspace = state.config.workspace_path();
        println!(
            "\n  Creating workspace at {}...\n",
            colorize(&workspace.display().to_string(), DIM)
        );

        // Create directories
        std::fs::create_dir_all(&workspace)?;
        std::fs::create_dir_all(workspace.join("memory"))?;
        std::fs::create_dir_all(workspace.join("skills"))?;
        println!("  {} Created workspace directories", status_success());

        // Create template files (only if they don't already exist)
        create_template_file(&workspace.join("AGENTS.md"), templates::AGENTS)?;
        create_template_file(&workspace.join("SOUL.md"), templates::SOUL)?;
        create_template_file(&workspace.join("USER.md"), templates::USER)?;
        create_template_file(&workspace.join("TOOLS.md"), templates::TOOLS)?;
        create_template_file(&workspace.join("IDENTITY.md"), templates::IDENTITY)?;
        println!("  {} Created workspace templates", status_success());

        // Create memory file
        let memory_file = workspace.join("memory").join("MEMORY.md");
        if !memory_file.exists() {
            std::fs::write(&memory_file, templates::MEMORY)?;
        }
        println!("  {} Initialized memory directory", status_success());

        // Create config directories
        let config_dir = config::config_dir()?;
        std::fs::create_dir_all(config_dir.join("sessions"))?;
        std::fs::create_dir_all(config_dir.join("cron"))?;
        std::fs::create_dir_all(config_dir.join("media"))?;
        std::fs::create_dir_all(config_dir.join("history"))?;
        println!("  {} Set up configuration directories", status_success());

        Ok(StepResult::Next)
    }
}

/// Create a template file if it doesn't already exist.
fn create_template_file(path: &std::path::Path, content: &str) -> Result<()> {
    if !path.exists() {
        std::fs::write(path, content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================================================
    // WorkspaceModule metadata tests
    // ========================================================================

    #[test]
    fn test_workspace_module_metadata() {
        let module = WorkspaceModule;
        assert_eq!(module.name(), "Workspace Setup");
        assert!(!module.description().is_empty());
        assert!(module.is_required()); // default is true
    }

    #[test]
    fn test_workspace_module_is_applicable() {
        let module = WorkspaceModule;
        let state = WizardState::new();
        assert!(module.is_applicable(&state));
    }

    // ========================================================================
    // create_template_file tests
    // ========================================================================

    #[test]
    fn test_create_template_file_creates_new() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.md");

        create_template_file(&path, "test content").unwrap();

        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "test content");
    }

    #[test]
    fn test_create_template_file_skips_existing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.md");

        std::fs::write(&path, "original").unwrap();
        create_template_file(&path, "replacement").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "original",
            "Should not overwrite existing file"
        );
    }

    #[test]
    fn test_create_template_file_with_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("empty.md");

        create_template_file(&path, "").unwrap();

        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn test_create_template_file_with_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("unicode.md");

        let content = "# 你好世界\n\nEmoji: 🤖✅❌\nCyrillc: Привет";
        create_template_file(&path, content).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }
}
