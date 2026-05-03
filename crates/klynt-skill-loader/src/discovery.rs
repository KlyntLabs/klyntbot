use crate::frontmatter::KlyntFrontmatter;
use crate::index::{DiscoveryRoots, IndexedSkill, SkillIndex, SkillSource};
use common::{ConfigError, KlyntbotError, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Trait for querying MCP server resources. Implemented by the real MCP client
/// in `app-core` and by fakes in tests.
#[async_trait::async_trait]
pub trait McpResourceClient: Send + Sync {
    /// Human-readable server name (e.g. "github").
    fn server_name(&self) -> &str;
    /// List all resources exposed by this MCP server.
    async fn list_resources(&self) -> Result<Vec<McpResource>>;
    /// Read the content of a resource by URI.
    async fn read_resource(&self, uri: &str) -> Result<String>;
}

/// A resource exposed by an MCP server.
#[derive(Debug, Clone)]
pub struct McpResource {
    pub uri: String,
    pub name: Option<String>,
}

impl SkillIndex {
    /// Discover all skills under the four static roots (§8.911-918).
    pub fn discover(roots: &DiscoveryRoots) -> Result<Self> {
        let mut idx = SkillIndex::new();
        idx.merge(scan_root(
            roots.klyntbot_home.join("skills"),
            SkillSource::User,
        )?);
        if let Some(repo_id) = &roots.repo_id {
            idx.merge(scan_root(
                roots
                    .klyntbot_home
                    .join("project-skills")
                    .join(sanitize_repo_id(repo_id)),
                SkillSource::ReforgePrivate,
            )?);
        }
        if let Some(repo_root) = &roots.repo_root {
            idx.merge(scan_root(
                repo_root.join(".klyntbot/skills"),
                SkillSource::Project,
            )?);
            idx.merge(scan_root(
                repo_root.join(".klyntbot/team-skills"),
                SkillSource::ReforgeTeam,
            )?);
        }
        let cwd_path = roots.cwd.join(".klyntbot/skills");
        if Some(&cwd_path)
            != roots
                .repo_root
                .as_ref()
                .map(|r| r.join(".klyntbot/skills"))
                .as_ref()
        {
            idx.merge(scan_root(cwd_path, SkillSource::Project)?);
        }
        Ok(idx)
    }
}

/// Path-safe repo-id (replaces `/` and `:` so a github URL becomes one segment).
pub fn sanitize_repo_id(repo_id: &str) -> String {
    repo_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn scan_root(dir: PathBuf, source: SkillSource) -> Result<SkillIndex> {
    if !dir.is_dir() {
        return Ok(SkillIndex::new());
    }
    let mut idx = SkillIndex::new();
    let entries = fs::read_dir(&dir).map_err(|e| {
        KlyntbotError::Config(ConfigError::Invalid(format!(
            "reading {}: {e}",
            dir.display()
        )))
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!(
                "reading {}: {e}",
                dir.display()
            )))
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        match parse_skill(&skill_md, source.clone()) {
            Ok(skill) => idx.insert(skill.frontmatter.name.clone(), skill),
            Err(e) => tracing::warn!(
                path = %skill_md.display(),
                error = %e,
                "skipping malformed SKILL.md"
            ),
        }
    }
    Ok(idx)
}

fn parse_skill(skill_md: &Path, source: SkillSource) -> Result<IndexedSkill> {
    let raw = fs::read_to_string(skill_md).map_err(|e| {
        KlyntbotError::Config(ConfigError::Invalid(format!(
            "reading {}: {e}",
            skill_md.display()
        )))
    })?;
    let (frontmatter, _body) = KlyntFrontmatter::parse(&raw)?;
    Ok(IndexedSkill {
        frontmatter,
        source,
        source_path: skill_md.to_path_buf(),
    })
}

/// Scan an MCP server for skills exposed via `klynt-skill://` resource URIs.
///
/// Resources with a `klynt-skill://` URI scheme are parsed as SKILL.md content
/// and returned as `IndexedSkill` entries with `SkillSource::Mcp`.
pub async fn scan_mcp_server<C: McpResourceClient>(client: &C) -> Result<SkillIndex> {
    let mut idx = SkillIndex::new();
    let resources = client.list_resources().await?;
    for r in resources {
        let Some(skill_name) = r.uri.strip_prefix("klynt-skill://") else {
            continue;
        };
        // Strip server prefix if present (e.g. "klynt-skill://github/code-review" → "code-review")
        let skill_name = skill_name.split('/').nth(1).unwrap_or(skill_name);
        match client.read_resource(&r.uri).await {
            Ok(body) => {
                let source = SkillSource::Mcp {
                    server_name: client.server_name().to_string(),
                };
                match KlyntFrontmatter::parse(&body) {
                    Ok((frontmatter, _body)) => {
                        let display_name = if frontmatter.name.is_empty() {
                            skill_name.to_string()
                        } else {
                            frontmatter.name.clone()
                        };
                        idx.insert(
                            display_name,
                            IndexedSkill {
                                frontmatter,
                                source,
                                source_path: PathBuf::from(format!(
                                    "mcp://{}/{skill_name}",
                                    client.server_name()
                                )),
                            },
                        );
                    }
                    Err(e) => tracing::warn!(
                        uri = %r.uri,
                        error = %e,
                        "skipping malformed MCP skill resource"
                    ),
                }
            }
            Err(e) => tracing::warn!(
                uri = %r.uri,
                error = %e,
                "failed to read MCP skill resource"
            ),
        }
    }
    Ok(idx)
}
