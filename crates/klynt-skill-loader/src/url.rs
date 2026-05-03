use common::{ConfigError, KlyntbotError, Result};

/// Classifies a skill URL into its kind for dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillUrlKind {
    GitHub {
        owner: String,
        repo: String,
        path: Option<String>,
    },
    Gist {
        id: String,
    },
    LocalPath(std::path::PathBuf),
    SkillsSh {
        name: String,
    },
}

/// Classify a skill URL string into a `SkillUrlKind`.
pub fn classify_skill_url(s: &str) -> Result<SkillUrlKind> {
    // skills.sh short URLs: https://skills.sh/<name>[@version]
    if let Some(rest) = s.strip_prefix("https://skills.sh/") {
        if rest.is_empty() || rest == "/" {
            return Err(KlyntbotError::Config(ConfigError::Invalid(
                "empty skill name in skills.sh URL".into(),
            )));
        }
        let name = rest.strip_suffix('/').unwrap_or(rest);
        return Ok(SkillUrlKind::SkillsSh {
            name: name.to_string(),
        });
    }

    // Local path
    let path = std::path::Path::new(s);
    if path.exists() {
        return Ok(SkillUrlKind::LocalPath(path.to_path_buf()));
    }

    // GitHub: https://github.com/owner/repo[/path]
    if let Some(gh) = s.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = gh.trim_end_matches('/').split('/').collect();
        if parts.len() >= 2 {
            return Ok(SkillUrlKind::GitHub {
                owner: parts[0].to_string(),
                repo: parts[1].to_string(),
                path: if parts.len() > 2 {
                    Some(parts[2..].join("/"))
                } else {
                    None
                },
            });
        }
    }

    // GitHub Gist: https://gist.github.com/user/id
    if let Some(gist) = s.strip_prefix("https://gist.github.com/") {
        let parts: Vec<&str> = gist.trim_end_matches('/').split('/').collect();
        if let Some(id) = parts.last() {
            return Ok(SkillUrlKind::Gist { id: id.to_string() });
        }
    }

    Err(KlyntbotError::Config(ConfigError::Invalid(format!(
        "unrecognized skill URL: {s}"
    ))))
}

/// Clone a skill from a Git URL into a temporary directory, then copy it
/// into `target_dir` (typically `~/.klyntbot/skills/<name>`).
///
/// Returns the installed skill name on success.
pub async fn load_from_url(url: &str, target_dir: &std::path::Path) -> Result<String> {
    let kind = classify_skill_url(url)?;
    match kind {
        SkillUrlKind::SkillsSh { name } => load_from_skills_sh(&name, target_dir).await,
        SkillUrlKind::GitHub { .. } | SkillUrlKind::Gist { .. } => {
            load_from_git(url, target_dir).await
        }
        SkillUrlKind::LocalPath(path) => load_from_local(&path, target_dir),
    }
}

/// Install a skill from skills.sh marketplace.
///
/// Parses `name[@version]`, fetches the tarball from the skills.sh API,
/// and extracts into `target_dir/<name>`.
async fn load_from_skills_sh(name: &str, target_dir: &std::path::Path) -> Result<String> {
    // Parse name@version (default: latest)
    let (skill_name, _version) = match name.split_once('@') {
        Some((n, v)) => (n, v),
        None => (name, "latest"),
    };

    // Fetch tarball URL from skills.sh API
    let api_url = format!("https://skills.sh/api/v1/skills/{skill_name}/latest");
    let resp = reqwest::get(&api_url).await.map_err(|e| {
        KlyntbotError::Config(ConfigError::Invalid(format!("skills.sh fetch: {e}")))
    })?;

    if !resp.status().is_success() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "skills.sh returned {} for skill '{}'",
            resp.status(),
            skill_name
        ))));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("skills.sh body: {e}"))))?;

    // Parse the JSON response to get the tarball content (base64-encoded)
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("skills.sh json: {e}"))))?;

    // The API returns the SKILL.md content directly
    let skill_md_content = json["skill_md"].as_str().ok_or_else(|| {
        KlyntbotError::Config(ConfigError::Invalid(
            "skills.sh response missing skill_md field".into(),
        ))
    })?;

    let (frontmatter, _body) = crate::frontmatter::KlyntFrontmatter::parse(skill_md_content)?;
    let dst = target_dir.join(&frontmatter.name);
    if dst.exists() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "skill `{}` already installed",
            frontmatter.name
        ))));
    }

    std::fs::create_dir_all(&dst)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("create dst: {e}"))))?;
    std::fs::write(dst.join("SKILL.md"), skill_md_content)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("write SKILL.md: {e}"))))?;

    Ok(frontmatter.name)
}

/// Clone from a git URL and install.
async fn load_from_git(url: &str, target_dir: &std::path::Path) -> Result<String> {
    let temp = tempfile::tempdir()
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("tempdir: {e}"))))?;
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, temp.path().to_str().unwrap()])
        .status()
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("git clone: {e}"))))?;
    if !status.success() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "git clone failed: {url}"
        ))));
    }

    let skill_md = temp.path().join("SKILL.md");
    if !skill_md.exists() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(
            "cloned repo missing SKILL.md".into(),
        )));
    }
    let raw = std::fs::read_to_string(&skill_md)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("read SKILL.md: {e}"))))?;
    let (frontmatter, _) = crate::frontmatter::KlyntFrontmatter::parse(&raw)?;
    let dst = target_dir.join(&frontmatter.name);
    if dst.exists() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "skill `{}` already installed",
            frontmatter.name
        ))));
    }

    fs_copy_dir_all(temp.path(), &dst)?;
    Ok(frontmatter.name)
}

/// Install from a local path.
fn load_from_local(src: &std::path::Path, target_dir: &std::path::Path) -> Result<String> {
    let skill_md = src.join("SKILL.md");
    if !skill_md.exists() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(
            "local skill missing SKILL.md".into(),
        )));
    }
    let raw = std::fs::read_to_string(&skill_md)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("read SKILL.md: {e}"))))?;
    let (frontmatter, _) = crate::frontmatter::KlyntFrontmatter::parse(&raw)?;
    let dst = target_dir.join(&frontmatter.name);
    if dst.exists() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "skill `{}` already installed",
            frontmatter.name
        ))));
    }

    fs_copy_dir_all(src, &dst)?;
    Ok(frontmatter.name)
}

fn fs_copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("create dst: {e}"))))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("read src: {e}"))))?
    {
        let entry = entry
            .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("read src: {e}"))))?;
        let ft = entry
            .file_type()
            .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("file_type: {e}"))))?;
        let dst_entry = dst.join(entry.file_name());
        if ft.is_dir() {
            fs_copy_dir_all(&entry.path(), &dst_entry)?;
        } else {
            std::fs::copy(entry.path(), &dst_entry)
                .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("copy: {e}"))))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_sh_url_recognized() {
        let k = classify_skill_url("https://skills.sh/rust-debugging").unwrap();
        assert!(matches!(k, SkillUrlKind::SkillsSh { name } if name == "rust-debugging"));
    }

    #[test]
    fn versioned_skills_sh_url() {
        let k = classify_skill_url("https://skills.sh/rust-debugging@1.2.0").unwrap();
        match k {
            SkillUrlKind::SkillsSh { name } => assert_eq!(name, "rust-debugging@1.2.0"),
            _ => panic!("expected SkillsSh"),
        }
    }

    #[test]
    fn skills_sh_url_with_trailing_slash() {
        let k = classify_skill_url("https://skills.sh/my-skill/").unwrap();
        assert!(matches!(k, SkillUrlKind::SkillsSh { name } if name == "my-skill"));
    }

    #[test]
    fn skills_sh_empty_name_errors() {
        assert!(classify_skill_url("https://skills.sh/").is_err());
        assert!(classify_skill_url("https://skills.sh").is_err());
    }

    #[test]
    fn github_url_recognized() {
        let k = classify_skill_url("https://github.com/owner/repo").unwrap();
        assert!(matches!(k, SkillUrlKind::GitHub { owner, repo, path }
                if owner == "owner" && repo == "repo" && path.is_none()));
    }

    #[test]
    fn github_url_with_path() {
        let k = classify_skill_url("https://github.com/owner/repo/path/to/skill").unwrap();
        assert!(matches!(k, SkillUrlKind::GitHub { owner, repo, path }
                if owner == "owner" && repo == "repo" && path.as_deref() == Some("path/to/skill")));
    }
}
