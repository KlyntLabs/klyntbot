//! Skills command handlers for managing agent skills

use anyhow::Result;
use klyntbot_agent::SkillManager;
use crate::SkillsCommands;

/// Handle skills commands
pub async fn handle_skills(cmd: SkillsCommands) -> Result<()> {
    match cmd {
        SkillsCommands::List => {
            let config = klyntbot_config::load()?;
            let workspace = config.workspace_path();

            let mut skill_manager = SkillManager::new();
            skill_manager.load(workspace).await?;

            let skills = skill_manager.all();

            if skills.is_empty() {
                println!("No skills found");
                println!(
                    "\nSkills directory: {}",
                    config.workspace_path().join("skills").display()
                );
                println!("Create skills with SKILL.md files in subdirectories");
                return Ok(());
            }

            println!("Available skills:\n");

            for skill in &skills {
                let status = if skill.available { "✓" } else { "✗" };
                println!("{} {}", status, skill.name);
                println!("   {}", skill.description);

                if !skill.requires_bins.is_empty() {
                    println!("   Requires: {}", skill.requires_bins.join(", "));
                }
                if !skill.requires_env.is_empty() {
                    println!("   Env vars: {}", skill.requires_env.join(", "));
                }
                if !skill.triggers.is_empty() {
                    println!("   Triggers: {}", skill.triggers.join(", "));
                }

                println!();
            }

            println!("Total: {} skill(s)", skills.len());
        }
        SkillsCommands::Info { name } => {
            let config = klyntbot_config::load()?;
            let workspace = config.workspace_path();

            let mut skill_manager = SkillManager::new();
            skill_manager.load(workspace).await?;

            match skill_manager.get(&name) {
                Some(skill) => {
                    println!("Skill: {}", skill.name);
                    println!("Description: {}", skill.description);
                    println!("Version: {}", skill.version);
                    println!("Available: {}", if skill.available { "yes" } else { "no" });
                    println!("Always load: {}", if skill.always { "yes" } else { "no" });

                    if !skill.triggers.is_empty() {
                        println!("Triggers: {}", skill.triggers.join(", "));
                    }

                    if !skill.requires_bins.is_empty() {
                        println!("Required binaries: {}", skill.requires_bins.join(", "));
                    }

                    if !skill.requires_env.is_empty() {
                        println!("Required env vars: {}", skill.requires_env.join(", "));
                    }

                    println!("\nPath: {}", skill.path.display());

                    if let Some(content) = &skill.content {
                        println!("\n--- Content ---");
                        println!("{}", content);
                    }
                }
                None => {
                    return Err(anyhow::anyhow!(
                        "Skill '{}' not found\n\nUse 'klyntbot skills list' to see all available skills",
                        name
                    ));
                }
            }
        }
        SkillsCommands::Path => {
            let config = klyntbot_config::load()?;
            let skills_path = config.workspace_path().join("skills");
            println!("{}", skills_path.display());
        }
    }
    Ok(())
}
