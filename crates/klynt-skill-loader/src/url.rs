use common::{ConfigError, KlyntbotError, Result};

/// Clone a skill from a Git URL into a temporary directory, then copy it
/// into `target_dir` (typically `~/.klyntbot/skills/<name>`).
///
/// Returns the installed skill name on success.
pub async fn load_from_url(url: &str, target_dir: &std::path::Path) -> Result<String> {
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

    // Find the skill name from the cloned frontmatter
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
