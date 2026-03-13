use desktop_shared::commands::{AgentFileContent, AgentFileSummary, AgentProfileSummary};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

/// Template for a new AGENT.md file.
const NEW_AGENT_TEMPLATE: &str = r#"---
name: {name}
description: Custom agent
tools: []
mcp_tools: []
triggers: []
max_iterations: 10
can_delegate_to: []
always_skills: []
---

You are the {name} agent. Describe your behavior here.
"#;

/// Template for a new skill file.
const NEW_SKILL_TEMPLATE: &str = r#"---
name: {name}
description: Describe this skill
metadata:
  author: user
  version: "1.0.0"
  updated-on: "{date}"
  source: custom
  tags: ""
  always: false
  triggers: ""
  agent: {agent}
---

Skill instructions here.
"#;

impl AppCore {
    /// List all agent profiles (built-in + workspace) with their files.
    pub async fn agent_list_profiles(&self) -> Result<Vec<AgentProfileSummary>, ApiError> {
        let workspace = self.config.read().await.workspace_path();
        let agents_dir = workspace.join("agents");

        // Start with built-in agents
        let builtins = agent::agent_profile::builtin_agents();
        let mut profiles: Vec<AgentProfileSummary> = Vec::new();

        for bi in &builtins {
            let has_override = agents_dir.join(bi.name).join("AGENT.md").exists();

            let mut files = vec![AgentFileSummary {
                filename: "AGENT.md".to_string(),
                display_name: "AGENT.md".to_string(),
                description: "Agent profile and configuration".to_string(),
                is_builtin: true,
                has_override,
            }];

            for skill in &bi.skills {
                let skill_override = agents_dir
                    .join(bi.name)
                    .join("skills")
                    .join(format!("{}.md", skill.name))
                    .exists();
                files.push(AgentFileSummary {
                    filename: format!("skills/{}.md", skill.name),
                    display_name: skill.name.to_string(),
                    description: extract_description(skill.content),
                    is_builtin: true,
                    has_override: skill_override,
                });
            }

            // Check for additional workspace-only skills
            let ws_skills_dir = agents_dir.join(bi.name).join("skills");
            if ws_skills_dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(&ws_skills_dir).await {
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
                        if bi.skills.iter().any(|s| s.name == stem) {
                            continue;
                        }
                        let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                        files.push(AgentFileSummary {
                            filename: format!("skills/{}.md", stem),
                            display_name: stem.to_string(),
                            description: extract_description(&content),
                            is_builtin: false,
                            has_override: false,
                        });
                    }
                }
            }

            // Parse description from built-in AGENT.md frontmatter
            let description = extract_description(bi.content);

            profiles.push(AgentProfileSummary {
                name: bi.name.to_string(),
                description,
                is_builtin: true,
                has_override,
                files,
            });
        }

        // Scan workspace for custom agents not in builtins
        if agents_dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&agents_dir).await {
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
                    let agent_md = path.join("AGENT.md");
                    if !agent_md.exists() {
                        continue;
                    }

                    let content = tokio::fs::read_to_string(&agent_md).await.unwrap_or_default();

                    let mut files = vec![AgentFileSummary {
                        filename: "AGENT.md".to_string(),
                        display_name: "AGENT.md".to_string(),
                        description: "Agent profile and configuration".to_string(),
                        is_builtin: false,
                        has_override: false,
                    }];

                    let skills_dir = path.join("skills");
                    if skills_dir.exists() {
                        if let Ok(mut skill_entries) = tokio::fs::read_dir(&skills_dir).await {
                            while let Ok(Some(se)) = skill_entries.next_entry().await {
                                let sp = se.path();
                                if sp.extension().and_then(|e| e.to_str()) != Some("md") {
                                    continue;
                                }
                                let stem = sp
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown");
                                let sc =
                                    tokio::fs::read_to_string(&sp).await.unwrap_or_default();
                                files.push(AgentFileSummary {
                                    filename: format!("skills/{}.md", stem),
                                    display_name: stem.to_string(),
                                    description: extract_description(&sc),
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

    /// Read an agent file (AGENT.md or skills/foo.md).
    /// Returns workspace override if present, falls back to built-in.
    pub async fn agent_read_file(
        &self,
        agent_name: &str,
        filename: &str,
    ) -> Result<AgentFileContent, ApiError> {
        validate_agent_filename(filename)?;

        let workspace = self.config.read().await.workspace_path();
        let ws_path = workspace.join("agents").join(agent_name).join(filename);

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
            if filename == "AGENT.md" {
                return Ok(AgentFileContent {
                    agent_name: agent_name.to_string(),
                    filename: filename.to_string(),
                    content: bi.content.to_string(),
                    is_builtin: true,
                });
            }
            // skills/foo.md
            if let Some(stripped) = filename.strip_prefix("skills/") {
                let skill_name = stripped.strip_suffix(".md").unwrap_or(stripped);
                if let Some(skill) = bi.skills.iter().find(|s| s.name == skill_name) {
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
            format!("Agent file not found: {agent_name}/{filename}"),
        ))
    }

    /// Write an agent file to the workspace directory and trigger hot-reload.
    pub async fn agent_write_file(
        &self,
        agent_name: &str,
        filename: &str,
        content: &str,
    ) -> Result<AgentFileContent, ApiError> {
        validate_agent_filename(filename)?;

        let workspace = self.config.read().await.workspace_path();
        let ws_path = workspace.join("agents").join(agent_name).join(filename);

        if let Some(parent) = ws_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        }

        tokio::fs::write(&ws_path, content)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload agent profiles
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Agent hot-reload failed: {e}");
        }

        Ok(AgentFileContent {
            agent_name: agent_name.to_string(),
            filename: filename.to_string(),
            content: content.to_string(),
            is_builtin: false,
        })
    }

    /// Create a new custom agent profile.
    pub async fn agent_create_profile(
        &self,
        name: &str,
    ) -> Result<AgentProfileSummary, ApiError> {
        let workspace = self.config.read().await.workspace_path();
        let agent_dir = workspace.join("agents").join(name);

        if agent_dir.join("AGENT.md").exists() {
            return Err(ApiError::new(
                "CONFLICT",
                format!("Agent '{name}' already exists"),
            ));
        }

        tokio::fs::create_dir_all(&agent_dir)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        tokio::fs::create_dir_all(agent_dir.join("skills"))
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        let content = NEW_AGENT_TEMPLATE
            .replace("{name}", name);
        tokio::fs::write(agent_dir.join("AGENT.md"), &content)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Agent hot-reload failed: {e}");
        }

        Ok(AgentProfileSummary {
            name: name.to_string(),
            description: "Custom agent".to_string(),
            is_builtin: false,
            has_override: false,
            files: vec![AgentFileSummary {
                filename: "AGENT.md".to_string(),
                display_name: "AGENT.md".to_string(),
                description: "Agent profile and configuration".to_string(),
                is_builtin: false,
                has_override: false,
            }],
        })
    }

    /// Create a new skill file for an agent.
    pub async fn agent_create_skill(
        &self,
        agent_name: &str,
        skill_name: &str,
    ) -> Result<AgentFileSummary, ApiError> {
        let workspace = self.config.read().await.workspace_path();
        let skill_path = workspace
            .join("agents")
            .join(agent_name)
            .join("skills")
            .join(format!("{skill_name}.md"));

        if skill_path.exists() {
            return Err(ApiError::new(
                "CONFLICT",
                format!("Skill '{skill_name}' already exists for agent '{agent_name}'"),
            ));
        }

        if let Some(parent) = skill_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        }

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let content = NEW_SKILL_TEMPLATE
            .replace("{name}", skill_name)
            .replace("{agent}", agent_name)
            .replace("{date}", &date);
        tokio::fs::write(&skill_path, &content)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Agent hot-reload failed: {e}");
        }

        Ok(AgentFileSummary {
            filename: format!("skills/{skill_name}.md"),
            display_name: skill_name.to_string(),
            description: "Describe this skill".to_string(),
            is_builtin: false,
            has_override: false,
        })
    }

    /// Delete a workspace agent file (skill only — cannot delete AGENT.md of built-in agents).
    pub async fn agent_delete_file(
        &self,
        agent_name: &str,
        filename: &str,
    ) -> Result<bool, ApiError> {
        validate_agent_filename(filename)?;

        let workspace = self.config.read().await.workspace_path();
        let ws_path = workspace.join("agents").join(agent_name).join(filename);

        if !ws_path.exists() {
            return Ok(false);
        }

        tokio::fs::remove_file(&ws_path)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        // Hot-reload
        if let Err(e) = self.agent.reload_agents().await {
            tracing::warn!("Agent hot-reload failed: {e}");
        }

        Ok(true)
    }
}

/// Validate that a filename is safe (no path traversal).
fn validate_agent_filename(filename: &str) -> Result<(), ApiError> {
    if filename.contains("..") || filename.starts_with('/') || filename.starts_with('\\') {
        return Err(ApiError::new(
            "INVALID_PARAMS",
            "Invalid filename: path traversal not allowed".to_string(),
        ));
    }
    // Must be AGENT.md or skills/*.md
    if filename == "AGENT.md" || (filename.starts_with("skills/") && filename.ends_with(".md")) {
        Ok(())
    } else {
        Err(ApiError::new(
            "INVALID_PARAMS",
            format!("Invalid agent filename: '{filename}'. Must be 'AGENT.md' or 'skills/*.md'"),
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
