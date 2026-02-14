//! Workspace setup wizard module.
//!
//! Creates the workspace directory structure and template files
//! (AGENTS.md, SOUL.md, USER.md, TOOLS.md, IDENTITY.md, MEMORY.md).
//! On reconfiguration, shows workspace path with current as default,
//! silently creates dirs/templates if missing.

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
        let chars = BoxChars::get();
        let current_path = state.config.workspace_path();

        // Show workspace path with current as default - Enter to keep
        let new_path = prompts::prompt_text(
            "Workspace path",
            Some(&current_path.display().to_string()),
            true,
        )?;

        // Update config if path changed
        if new_path != current_path.display().to_string() {
            state.config.agents.defaults.workspace = new_path;
        }

        let workspace = state.config.workspace_path();
        println!("{}", colorize(chars.vertical, BRAND));

        // Create directories (silently, no y/n)
        std::fs::create_dir_all(&workspace)?;
        std::fs::create_dir_all(workspace.join("memory"))?;
        std::fs::create_dir_all(workspace.join("skills"))?;

        // Create template files (only if they don't already exist)
        let mut template_count = 0;
        template_count += create_template_file(&workspace.join("AGENTS.md"), templates::AGENTS)?;
        template_count += create_template_file(&workspace.join("SOUL.md"), templates::SOUL)?;
        template_count += create_template_file(&workspace.join("USER.md"), templates::USER)?;
        template_count += create_template_file(&workspace.join("TOOLS.md"), templates::TOOLS)?;
        template_count +=
            create_template_file(&workspace.join("IDENTITY.md"), templates::IDENTITY)?;

        // Create memory file
        let memory_file = workspace.join("memory").join("MEMORY.md");
        if !memory_file.exists() {
            std::fs::write(&memory_file, templates::MEMORY)?;
            template_count += 1;
        }

        // Create config directories
        let config_dir = config::config_dir()?;
        std::fs::create_dir_all(config_dir.join("sessions"))?;
        std::fs::create_dir_all(config_dir.join("cron"))?;
        std::fs::create_dir_all(config_dir.join("media"))?;
        std::fs::create_dir_all(config_dir.join("history"))?;

        // Report what was done
        println!(
            "{} {} Workspace directories verified",
            colorize(chars.vertical, BRAND),
            status_success()
        );

        if template_count > 0 {
            println!(
                "{} {} Created {} new template file(s)",
                colorize(chars.vertical, BRAND),
                status_success(),
                template_count
            );
        } else {
            println!(
                "{} {} Template files intact (5 files)",
                colorize(chars.vertical, BRAND),
                status_success()
            );
        }

        Ok(StepResult::Next)
    }
}

/// Create a template file if it doesn't already exist.
/// Returns 1 if created, 0 if already existed.
fn create_template_file(path: &std::path::Path, content: &str) -> Result<usize> {
    if !path.exists() {
        std::fs::write(path, content)?;
        Ok(1)
    } else {
        Ok(0)
    }
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

        let count = create_template_file(&path, "test content").unwrap();

        assert!(path.exists());
        assert_eq!(count, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "test content");
    }

    #[test]
    fn test_create_template_file_skips_existing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.md");

        std::fs::write(&path, "original").unwrap();
        let count = create_template_file(&path, "replacement").unwrap();

        assert_eq!(count, 0);
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

        let count = create_template_file(&path, "").unwrap();

        assert!(path.exists());
        assert_eq!(count, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn test_create_template_file_with_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("unicode.md");

        let content = "# 你好世界\n\nEmoji: 🤖✅❌\nCyrillc: Привет";
        let count = create_template_file(&path, content).unwrap();

        assert_eq!(count, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }
}
