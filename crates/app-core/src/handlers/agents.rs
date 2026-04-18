use desktop_shared::commands::{AgentFileContent, AgentFileSummary, AgentProfileSummary};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

/// Template for a new SKILL.md file.
const NEW_SKILL_MD_TEMPLATE: &str = r#"---
name: {name}
description: Custom skill
metadata:
  klyntbot:
    type: orchestrator
    max_iterations: 10
---

You are the {name} agent. Describe your behavior here.
"#;

/// Template for a new reference file.
const NEW_REFERENCE_TEMPLATE: &str = r#"---
name: {name}
description: Describe this reference
metadata:
  author: user
  version: "1.0.0"
  updated-on: "{date}"
  source: custom
---

Reference instructions here.
"#;

impl AppCore {
    /// List all skill profiles (built-in + workspace) with their files.
    pub async fn agent_list_profiles(&self) -> Result<Vec<AgentProfileSummary>, ApiError> {
        let workspace = self.config.read().await.workspace_path();
        let skills_dir = workspace.join("skills");

        // Start with built-in skills
        let builtins = agent::agent_profile::builtin_agents();
        let mut profiles: Vec<AgentProfileSummary> = Vec::new();

        for bi in &builtins {
            let has_override = skills_dir.join(bi.name).join("SKILL.md").exists();

            let mut files = vec![AgentFileSummary {
                filename: "SKILL.md".to_string(),
                display_name: "SKILL.md".to_string(),
                description: "Skill profile and configuration".to_string(),
                is_builtin: true,
                has_override,
            }];

            for skill in &bi.skills {
                let ref_override = skills_dir
                    .join(bi.name)
                    .join("references")
                    .join(format!("{}.md", skill.name))
                    .exists();
                files.push(AgentFileSummary {
                    filename: format!("references/{}.md", skill.name),
                    display_name: skill.name.to_string(),
                    description: extract_description(skill.content),
                    is_builtin: true,
                    has_override: ref_override,
                });
            }

            // Check for additional workspace-only references
            let ws_refs_dir = skills_dir.join(bi.name).join("references");
            if ws_refs_dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(&ws_refs_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) != Some("md") {
                            continue;
                        }
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown");
                        // Skip if already in built-in list
                        if bi.skills.iter().any(|r| r.name == stem) {
                            continue;
                        }
                        let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                        files.push(AgentFileSummary {
                            filename: format!("references/{}.md", stem),
                            display_name: stem.to_string(),
                            description: extract_description(&content),
                            is_builtin: false,
                            has_override: false,
                        });
                    }
                }
            }

            // Parse description from built-in SKILL.md frontmatter
            let description = extract_description(bi.content);

            profiles.push(AgentProfileSummary {
                name: bi.name.to_string(),
                description,
                is_builtin: true,
                has_override,
                files,
            });
        }

        // Scan workspace for custom skills not in builtins
        if skills_dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&skills_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    // Skip if already listed as builtin
                    if builtins.iter().any(|b| b.name == name) {
                        continue;
                    }
                    let skill_md = path.join("SKILL.md");
                    if !skill_md.exists() {
                        continue;
                    }

                    let content = tokio::fs::read_to_string(&skill_md)
                        .await
                        .unwrap_or_default();

                    let mut files = vec![AgentFileSummary {
                        filename: "SKILL.md".to_string(),
                        display_name: "SKILL.md".to_string(),
                        description: "Skill profile and configuration".to_string(),
                        is_builtin: false,
                        has_override: false,
                    }];

                    let refs_dir = path.join("references");
                    if refs_dir.exists() {
                        if let Ok(mut ref_entries) = tokio::fs::read_dir(&refs_dir).await {
                            while let Ok(Some(re)) = ref_entries.next_entry().await {
                                let rp = re.path();
                                if rp.extension().and_then(|e| e.to_str()) != Some("md") {
                                    continue;
                                }
                                let stem =
                                    rp.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                                let rc = tokio::fs::read_to_string(&rp).await.unwrap_or_default();
                                files.push(AgentFileSummary {
                                    filename: format!("references/{}.md", stem),
                                    display_name: stem.to_string(),
                                    description: extract_description(&rc),
                                    is_builtin: false,
                                    has_override: false,
                                });
                            }
                        }
                    }

                    profiles.push(AgentProfileSummary {
                        name,
                        description: extract_description(&content),
                        is_builtin: false,
                        has_override: false,
                        files,
                    });
                }
            }
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// Read a skill file (SKILL.md or references/foo.md).
    /// Returns workspace override if present, falls back to built-in.
    pub async fn agent_read_file(
        &self,
        agent_name: &str,
        filename: &str,
    ) -> Result<AgentFileContent, ApiError> {
        validate_skill_filename(filename)?;

        let workspace = self.config.read().await.workspace_path();
        let ws_path = workspace.join("skills").join(agent_name).join(filename);

        // Try workspace override first
        if ws_path.exists() {
            let content = tokio::fs::read_to_string(&ws_path)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
            return Ok(AgentFileContent {
                agent_name: agent_name.to_string(),
                filename: filename.to_string(),
                content,
                is_builtin: false,
            });
        }

        // Fall back to built-in
        let builtins = agent::agent_profile::builtin_agents();
        let bi = builtins.iter().find(|b| b.name == agent_name);

        if let Some(bi) = bi {
            if filename == "SKILL.md" {
                return Ok(AgentFileContent {
                    agent_name: agent_name.to_string(),
                    filename: filename.to_string(),
                    content: bi.content.to_string(),
                    is_builtin: true,
                });
            }
            // references/foo.md
            if let Some(stripped) = filename.strip_prefix("references/") {
                let ref_name = stripped.strip_suffix(".md").unwrap_or(stripped);
                if let Some(skill) = bi.skills.iter().find(|s| s.name == ref_name) {
                    return Ok(AgentFileContent {
                        agent_name: agent_name.to_string(),
                        filename: filename.to_string(),
                        content: skill.content.to_string(),
                        is_builtin: true,
                    });
                }
            }
        }

        Err(ApiError::new(
            "NOT_FOUND",
            format!("Skill file not found: {agent_name}/{filename}"),
        ))
    }

    /// Write a skill file to the workspace directory and trigger hot-reload.
    pub async fn agent_write_file(
        &self,
        agent_name: &str,
        filename: &str,
        content: &str,
    ) -> Result<AgentFileContent, ApiError> {
        validate_skill_filename(filename)?;

        let workspace = self.config.read().await.workspace_path();
        let ws_path = workspace.join("skills").join(agent_name).join(filename);

        if let Some(parent) = ws_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        }

        tokio::fs::write(&ws_path, content)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload skill packages
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Skill hot-reload failed: {e}");
        }

        Ok(AgentFileContent {
            agent_name: agent_name.to_string(),
            filename: filename.to_string(),
            content: content.to_string(),
            is_builtin: false,
        })
    }

    /// Create a new custom skill profile.
    pub async fn agent_create_profile(&self, name: &str) -> Result<AgentProfileSummary, ApiError> {
        let workspace = self.config.read().await.workspace_path();
        let skill_dir = workspace.join("skills").join(name);

        if skill_dir.join("SKILL.md").exists() {
            return Err(ApiError::new(
                "CONFLICT",
                format!("Skill '{name}' already exists"),
            ));
        }

        tokio::fs::create_dir_all(&skill_dir)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        tokio::fs::create_dir_all(skill_dir.join("references"))
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        let content = NEW_SKILL_MD_TEMPLATE.replace("{name}", name);
        tokio::fs::write(skill_dir.join("SKILL.md"), &content)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Skill hot-reload failed: {e}");
        }

        Ok(AgentProfileSummary {
            name: name.to_string(),
            description: "Custom skill".to_string(),
            is_builtin: false,
            has_override: false,
            files: vec![AgentFileSummary {
                filename: "SKILL.md".to_string(),
                display_name: "SKILL.md".to_string(),
                description: "Skill profile and configuration".to_string(),
                is_builtin: false,
                has_override: false,
            }],
        })
    }

    /// Create a new reference file for a skill.
    pub async fn agent_create_skill(
        &self,
        agent_name: &str,
        skill_name: &str,
    ) -> Result<AgentFileSummary, ApiError> {
        let workspace = self.config.read().await.workspace_path();
        let ref_path = workspace
            .join("skills")
            .join(agent_name)
            .join("references")
            .join(format!("{skill_name}.md"));

        if ref_path.exists() {
            return Err(ApiError::new(
                "CONFLICT",
                format!("Reference '{skill_name}' already exists for skill '{agent_name}'"),
            ));
        }

        if let Some(parent) = ref_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        }

        let date = jiff::Zoned::now().strftime("%Y-%m-%d").to_string();
        let content = NEW_REFERENCE_TEMPLATE
            .replace("{name}", skill_name)
            .replace("{date}", &date);
        tokio::fs::write(&ref_path, &content)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Skill hot-reload failed: {e}");
        }

        Ok(AgentFileSummary {
            filename: format!("references/{skill_name}.md"),
            display_name: skill_name.to_string(),
            description: "Describe this reference".to_string(),
            is_builtin: false,
            has_override: false,
        })
    }

    /// Delete a workspace skill file (reference only — cannot delete SKILL.md of built-in skills).
    pub async fn agent_delete_file(
        &self,
        agent_name: &str,
        filename: &str,
    ) -> Result<bool, ApiError> {
        validate_skill_filename(filename)?;

        let workspace = self.config.read().await.workspace_path();
        let ws_path = workspace.join("skills").join(agent_name).join(filename);

        if !ws_path.exists() {
            return Ok(false);
        }

        tokio::fs::remove_file(&ws_path)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Skill hot-reload failed: {e}");
        }

        Ok(true)
    }
}

/// Validate that a filename is safe (no path traversal).
fn validate_skill_filename(filename: &str) -> Result<(), ApiError> {
    if filename.contains("..") || filename.starts_with('/') || filename.starts_with('\\') {
        return Err(ApiError::new(
            "INVALID_PARAMS",
            "Invalid filename: path traversal not allowed".to_string(),
        ));
    }
    // Must be SKILL.md or references/*.md
    if filename == "SKILL.md" || (filename.starts_with("references/") && filename.ends_with(".md"))
    {
        Ok(())
    } else {
        Err(ApiError::new(
            "INVALID_PARAMS",
            format!(
                "Invalid skill filename: '{filename}'. Must be 'SKILL.md' or 'references/*.md'"
            ),
        ))
    }
}

/// Extract description from YAML frontmatter.
fn extract_description(content: &str) -> String {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return String::new();
    }
    let after = &trimmed[3..];
    if let Some(end) = after.find("\n---") {
        let fm = &after[..end];
        for line in fm.lines() {
            let line = line.trim();
            if let Some(desc) = line.strip_prefix("description:") {
                return desc.trim().trim_matches('"').trim_matches('\'').to_string();
            }
        }
    }
    String::new()
}
