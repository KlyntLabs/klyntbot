//! Runtime skill loading from filesystem.
//!
//! Discovers `.md` skill files in a directory, parses their frontmatter,
//! and makes them available for agent profile augmentation.

use std::path::PathBuf;

use crate::agent_profile::AgentSkill;

pub struct SkillLoader {
    skills_dir: PathBuf,
    external_skills: Vec<AgentSkill>,
}

impl SkillLoader {
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            external_skills: Vec::new(),
        }
    }

    /// Load all `.md` skill files from the configured directory.
    /// Returns an empty vec if the directory doesn't exist.
    fn load_external_skills(&self) -> common::Result<Vec<AgentSkill>> {
        if !self.skills_dir.exists() {
            return Ok(Vec::new());
        }
        let mut skills = Vec::new();
        for entry in std::fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let content = std::fs::read_to_string(&path)?;
                let file_stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                match AgentSkill::parse(file_stem, &content) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        tracing::warn!("Failed to parse skill {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(skills)
    }

    /// Re-scan the skills directory and update the cached list.
    pub fn refresh(&mut self) -> common::Result<()> {
        self.external_skills = self.load_external_skills()?;
        Ok(())
    }

    /// Get skills assigned to a specific agent.
    pub fn skills_for_agent(&self, agent_name: &str) -> Vec<&AgentSkill> {
        self.external_skills
            .iter()
            .filter(|s| s.agent.as_deref() == Some(agent_name))
            .collect()
    }

    /// Get all loaded external skills.
    pub fn all_external_skills(&self) -> &[AgentSkill] {
        &self.external_skills
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_skills_from_directory() {
        let dir = TempDir::new().unwrap();
        let skill_path = dir.path().join("test-skill.md");
        fs::write(
            &skill_path,
            r#"---
name: test-skill
description: A test skill
metadata:
  author: test
  version: "1.0.0"
  tags: "test,example"
---

Test skill content.
"#,
        )
        .unwrap();

        let mut loader = SkillLoader::new(dir.path().to_path_buf());
        loader.refresh().unwrap();
        let skills = loader.all_external_skills();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test-skill");
        assert_eq!(skills[0].tags, vec!["test", "example"]);
    }

    #[test]
    fn test_load_skills_empty_directory() {
        let dir = TempDir::new().unwrap();
        let mut loader = SkillLoader::new(dir.path().to_path_buf());
        loader.refresh().unwrap();
        assert!(loader.all_external_skills().is_empty());
    }

    #[test]
    fn test_load_skills_nonexistent_directory() {
        let mut loader = SkillLoader::new("/nonexistent/path".into());
        loader.refresh().unwrap();
        assert!(loader.all_external_skills().is_empty());
    }

    #[test]
    fn test_skills_for_agent() {
        let dir = TempDir::new().unwrap();

        fs::write(
            dir.path().join("task-skill.md"),
            r#"---
name: task-skill
description: Task skill
metadata:
  agent: task
---
Content.
"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("general-skill.md"),
            r#"---
name: general-skill
description: General skill
metadata:
  agent: general
---
Content.
"#,
        )
        .unwrap();

        let mut loader = SkillLoader::new(dir.path().to_path_buf());
        loader.refresh().unwrap();

        let task_skills = loader.skills_for_agent("task");
        assert_eq!(task_skills.len(), 1);
        assert_eq!(task_skills[0].name, "task-skill");

        let general_skills = loader.skills_for_agent("general");
        assert_eq!(general_skills.len(), 1);
        assert_eq!(general_skills[0].name, "general-skill");
    }
}
