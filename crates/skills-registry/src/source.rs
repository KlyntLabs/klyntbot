use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SkillSource {
    #[serde(rename_all = "camelCase")]
    Github { owner: String, repo: String, subpath: String, r#ref: GitRef },
    #[serde(rename_all = "camelCase")]
    SkillsSh { slug: String },
    #[serde(rename_all = "camelCase")]
    LocalPath { path: PathBuf },
    #[serde(rename_all = "camelCase")]
    Bundled { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GitRef {
    Latest,
    Tag { tag: String },
    Commit { sha: String },
}

impl SkillSource {
    /// Parse a user-entered string such as `owner/repo/subpath` or a full URL.
    pub fn parse_shorthand(input: &str) -> Result<Self, ParseError> {
        let trimmed = input.trim().trim_end_matches('/');
        // GitHub URL form
        if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
            return parse_github_path(rest);
        }
        // `owner/repo[/subpath]` form
        parse_github_path(trimmed)
    }
}

fn parse_github_path(path: &str) -> Result<SkillSource, ParseError> {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return Err(ParseError::BadFormat("expected owner/repo[/subpath]".into()));
    }
    let owner = parts[0].to_string();
    let repo = parts[1].to_string();
    let subpath = parts[2..].join("/");
    Ok(SkillSource::Github {
        owner,
        repo,
        subpath,
        r#ref: GitRef::Latest,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("bad source format: {0}")]
    BadFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_owner_repo_subpath() {
        let s = SkillSource::parse_shorthand("anthropics/skills/frontend-design").unwrap();
        match s {
            SkillSource::Github { owner, repo, subpath, r#ref } => {
                assert_eq!(owner, "anthropics");
                assert_eq!(repo, "skills");
                assert_eq!(subpath, "frontend-design");
                assert_eq!(r#ref, GitRef::Latest);
            }
            _ => panic!("expected github"),
        }
    }

    #[test]
    fn parse_full_github_url() {
        let s = SkillSource::parse_shorthand("https://github.com/anthropics/skills/").unwrap();
        match s {
            SkillSource::Github { owner, repo, subpath, .. } => {
                assert_eq!(owner, "anthropics");
                assert_eq!(repo, "skills");
                assert_eq!(subpath, "");
            }
            _ => panic!("expected github"),
        }
    }

    #[test]
    fn reject_invalid() {
        assert!(SkillSource::parse_shorthand("onlyone").is_err());
    }
}
