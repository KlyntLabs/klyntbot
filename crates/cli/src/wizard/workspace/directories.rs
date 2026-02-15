//! Workspace filesystem creation helpers.
//!
//! Creates workspace directories and template files on disk.

use std::path::Path;

use anyhow::Result;

use crate::wizard::templates;

/// Create workspace directories (workspace root, memory, skills).
pub(super) fn create_workspace_dirs(workspace: &Path) -> Result<()> {
    std::fs::create_dir_all(workspace)?;
    std::fs::create_dir_all(workspace.join("memory"))?;
    std::fs::create_dir_all(workspace.join("skills"))?;
    Ok(())
}

/// Create workspace template files (only if they don't already exist).
/// Returns the number of files newly created.
pub(super) fn create_workspace_templates(workspace: &Path) -> Result<usize> {
    let mut count = 0;
    count += create_template_file(&workspace.join("AGENTS.md"), templates::AGENTS)?;
    count += create_template_file(&workspace.join("SOUL.md"), templates::SOUL)?;
    count += create_template_file(&workspace.join("USER.md"), templates::USER)?;
    count += create_template_file(&workspace.join("TOOLS.md"), templates::TOOLS)?;
    count += create_template_file(&workspace.join("IDENTITY.md"), templates::IDENTITY)?;

    let memory_file = workspace.join("memory").join("MEMORY.md");
    if !memory_file.exists() {
        std::fs::write(&memory_file, templates::MEMORY)?;
        count += 1;
    }

    Ok(count)
}

/// Create config directories (sessions, cron, media, history).
pub(super) fn create_config_dirs() -> Result<()> {
    let config_dir = config::config_dir()?;
    std::fs::create_dir_all(config_dir.join("sessions"))?;
    std::fs::create_dir_all(config_dir.join("cron"))?;
    std::fs::create_dir_all(config_dir.join("media"))?;
    std::fs::create_dir_all(config_dir.join("history"))?;
    Ok(())
}

/// Create a template file if it doesn't already exist.
/// Returns 1 if created, 0 if already existed.
pub(super) fn create_template_file(path: &std::path::Path, content: &str) -> Result<usize> {
    if !path.exists() {
        std::fs::write(path, content)?;
        Ok(1)
    } else {
        Ok(0)
    }
}
