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

    // Copy built-in skills to workspace/skills/
    count += create_builtin_skills(workspace)?;

    Ok(count)
}

/// Copy built-in skills to workspace/skills/ (only if they don't already exist).
/// Returns the number of skills newly created.
fn create_builtin_skills(workspace: &Path) -> Result<usize> {
    let skills_dir = workspace.join("skills");
    let mut count = 0;

    // Built-in skills to copy (paths relative to workspace root)
    let builtin_skills = [
        ("cron", include_str!("../../../../../skills/cron/SKILL.md")),
        ("daily-planning", include_str!("../../../../../skills/daily-planning/SKILL.md")),
        ("github", include_str!("../../../../../skills/github/SKILL.md")),
        ("skill-creator", include_str!("../../../../../skills/skill-creator/SKILL.md")),
        ("summarize", include_str!("../../../../../skills/summarize/SKILL.md")),
        ("tmux", include_str!("../../../../../skills/tmux/SKILL.md")),
        ("todo", include_str!("../../../../../skills/todo/SKILL.md")),
        ("todo-party", include_str!("../../../../../skills/todo-party/SKILL.md")),
        ("todo-yolo", include_str!("../../../../../skills/todo-yolo/SKILL.md")),
        ("weather", include_str!("../../../../../skills/weather/SKILL.md")),
    ];

    for (name, content) in builtin_skills {
        let skill_dir = skills_dir.join(name);
        let skill_file = skill_dir.join("SKILL.md");

        if !skill_file.exists() {
            std::fs::create_dir_all(&skill_dir)?;
            std::fs::write(&skill_file, content)?;
            count += 1;
        }
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
