use std::path::PathBuf;

use common::{KlyntbotError, Result};
use serde_json::Value;
use tracing::debug;

use skill_system::parser::parse_skill_md;
use skill_system::store::split_frontmatter;
use skill_system::types::SkillScope;

use crate::package::{ReferenceFile, TemplateFile};
use crate::{GitRef, SkillPackage, SkillSource};

pub struct Fetcher {
    http: reqwest::Client,
    /// Override base URL for GitHub API — for tests.
    github_api_base: String,
    /// Override base URL for raw.githubusercontent — for tests.
    github_raw_base: String,
}

impl Fetcher {
    pub fn new() -> Self {
        Self::with_bases(
            "https://api.github.com".into(),
            "https://raw.githubusercontent.com".into(),
        )
    }

    pub fn with_bases(api: String, raw: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("klyntbot-skills-registry")
                .build()
                .expect("reqwest client"),
            github_api_base: api,
            github_raw_base: raw,
        }
    }

    pub async fn fetch(&self, source: &SkillSource) -> Result<SkillPackage> {
        match source {
            SkillSource::Github { owner, repo, subpath, r#ref } => {
                self.fetch_github(owner, repo, subpath, r#ref).await
            }
            SkillSource::LocalPath { path } => self.fetch_local(path.clone()).await,
            SkillSource::SkillsSh { slug } => {
                // Resolve slug → Github source (slugs follow owner/repo/subpath).
                let github = SkillSource::parse_shorthand(slug)
                    .map_err(|e| KlyntbotError::Storage(format!("bad skills.sh slug: {e}")))?;
                // Flatten to avoid recursive async fn — SkillsSh must resolve to Github.
                match github {
                    SkillSource::Github { owner, repo, subpath, r#ref } => {
                        self.fetch_github(&owner, &repo, &subpath, &r#ref).await
                    }
                    _ => Err(KlyntbotError::Storage(
                        "skills.sh slug must resolve to a github source".into(),
                    )),
                }
            }
            SkillSource::Bundled { name } => Err(KlyntbotError::Storage(format!(
                "bundled skill '{name}' is fetched directly from SkillStore"
            ))),
        }
    }

    async fn fetch_github(
        &self,
        owner: &str,
        repo: &str,
        subpath: &str,
        ref_: &GitRef,
    ) -> Result<SkillPackage> {
        // Resolve ref to a concrete SHA.
        let sha = self.resolve_ref(owner, repo, ref_).await?;

        // Fetch SKILL.md content.
        let skill_path = if subpath.is_empty() {
            "SKILL.md".into()
        } else {
            format!("{subpath}/SKILL.md")
        };
        let skill_md = self.fetch_raw(owner, repo, &sha, &skill_path).await?;

        let (frontmatter, _body) = split_frontmatter(&skill_md)
            .map_err(|e| KlyntbotError::Storage(format!("split_frontmatter: {e}")))?;

        let klyntbot_meta = parse_skill_md(&skill_md, PathBuf::from(&skill_path), SkillScope::User)
            .ok()
            .and_then(|pkg| pkg.metadata.klyntbot.clone());

        let semver = None;

        // Fetch references/ and templates/ directory listings (optional — may 404).
        let references = self
            .fetch_dir_files(
                owner,
                repo,
                &sha,
                &format!("{}/references", trim_trailing(subpath)),
            )
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(path, content)| ReferenceFile { path: PathBuf::from(path), content })
            .collect();

        let templates_raw = self
            .fetch_dir_files(
                owner,
                repo,
                &sha,
                &format!("{}/templates", trim_trailing(subpath)),
            )
            .await
            .unwrap_or_default();
        let mut templates = Vec::new();
        for (path, content) in templates_raw {
            if path.ends_with(".json") {
                let manifest: Value = serde_json::from_str(&content)
                    .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
                let name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("template.json")
                    .to_string();
                templates.push(TemplateFile { name, manifest });
            }
        }

        Ok(SkillPackage {
            name: frontmatter.name.clone(),
            source: SkillSource::Github {
                owner: owner.to_string(),
                repo: repo.to_string(),
                subpath: subpath.to_string(),
                r#ref: GitRef::Commit { sha: sha.clone() },
            },
            resolved_sha: sha,
            semver,
            skill_md_content: skill_md,
            frontmatter,
            klyntbot_meta,
            references,
            templates,
        })
    }

    async fn resolve_ref(&self, owner: &str, repo: &str, ref_: &GitRef) -> Result<String> {
        match ref_ {
            GitRef::Commit { sha } => Ok(sha.clone()),
            GitRef::Tag { tag } => {
                let url = format!(
                    "{}/repos/{}/{}/commits/{}",
                    self.github_api_base, owner, repo, tag
                );
                self.fetch_sha_from_commits_api(&url).await
            }
            GitRef::Latest => {
                let url = format!(
                    "{}/repos/{}/{}/commits/HEAD",
                    self.github_api_base, owner, repo
                );
                self.fetch_sha_from_commits_api(&url).await
            }
        }
    }

    async fn fetch_sha_from_commits_api(&self, url: &str) -> Result<String> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| KlyntbotError::Storage(format!("github GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!(
                "github {url}: HTTP {}",
                resp.status()
            )));
        }
        let json: Value =
            resp.json().await.map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        json.get("sha")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| KlyntbotError::Storage("missing sha field".into()))
    }

    async fn fetch_raw(&self, owner: &str, repo: &str, sha: &str, path: &str) -> Result<String> {
        let url = format!("{}/{}/{}/{}/{}", self.github_raw_base, owner, repo, sha, path);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| KlyntbotError::Storage(format!("raw GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!(
                "raw {url}: HTTP {}",
                resp.status()
            )));
        }
        resp.text().await.map_err(|e| KlyntbotError::Storage(e.to_string()))
    }

    /// List files in a directory via the GitHub contents API.
    /// Returns (relative_path, content_utf8).
    async fn fetch_dir_files(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        dir: &str,
    ) -> Result<Vec<(String, String)>> {
        let url = format!(
            "{}/repos/{}/{}/contents/{}?ref={}",
            self.github_api_base, owner, repo, dir, sha
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| KlyntbotError::Storage(format!("contents {url}: {e}")))?;
        if resp.status().as_u16() == 404 {
            return Ok(vec![]);
        }
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!(
                "contents {url}: HTTP {}",
                resp.status()
            )));
        }
        let items: Vec<Value> =
            resp.json().await.map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for item in items {
            let Some(kind) = item.get("type").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            if kind != "file" {
                continue;
            }
            let full_path = if dir.is_empty() {
                name.to_string()
            } else {
                format!("{dir}/{name}")
            };
            let content = self.fetch_raw(owner, repo, sha, &full_path).await?;
            out.push((name.to_string(), content));
        }
        debug!(dir = %dir, count = out.len(), "fetched dir files");
        Ok(out)
    }

    async fn fetch_local(&self, path: PathBuf) -> Result<SkillPackage> {
        let skill_md_path = path.join("SKILL.md");
        let skill_md = tokio::fs::read_to_string(&skill_md_path).await.map_err(|e| {
            KlyntbotError::Storage(format!("read {}: {e}", skill_md_path.display()))
        })?;
        let (frontmatter, _) = split_frontmatter(&skill_md)
            .map_err(|e| KlyntbotError::Storage(format!("split_frontmatter: {e}")))?;
        let klyntbot_meta =
            parse_skill_md(&skill_md, skill_md_path.clone(), SkillScope::User)
                .ok()
                .and_then(|pkg| pkg.metadata.klyntbot.clone());
        let semver = None;

        // Best-effort local references + templates.
        let references = collect_local_files(&path.join("references"))
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(p, c)| ReferenceFile { path: p, content: c })
            .collect();

        let mut templates = Vec::new();
        if let Ok(tpls) = collect_local_files(&path.join("templates")).await {
            for (p, c) in tpls {
                if p.extension().and_then(|e| e.to_str()) == Some("json") {
                    let manifest: Value =
                        serde_json::from_str(&c).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
                    templates.push(TemplateFile {
                        name: p
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("t.json")
                            .to_string(),
                        manifest,
                    });
                }
            }
        }

        let sha = compute_local_sha(&skill_md);
        Ok(SkillPackage {
            name: frontmatter.name.clone(),
            source: SkillSource::LocalPath { path },
            resolved_sha: sha,
            semver,
            skill_md_content: skill_md,
            frontmatter,
            klyntbot_meta,
            references,
            templates,
        })
    }
}

fn trim_trailing(s: &str) -> &str {
    s.trim_end_matches('/')
}

fn compute_local_sha(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("local-{}", hex::encode(h.finalize()))
}

async fn collect_local_files(dir: &std::path::Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let mut read_dir = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("read_dir {}: {e}", dir.display())))?;
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| KlyntbotError::Storage(e.to_string()))?
    {
        let path = entry.path();
        if path.is_file() {
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| KlyntbotError::Storage(format!("read {}: {e}", path.display())))?;
            out.push((path, content));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn local_fetch_reads_skill_md_and_templates() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("SKILL.md"),
            "---\nname: demo\ndescription: d\n---\nbody",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("templates")).unwrap();
        std::fs::write(
            tmp.path().join("templates/t.json"),
            r#"{"name":"t","fields":[]}"#,
        )
        .unwrap();

        let f = Fetcher::new();
        let pkg = f
            .fetch(&SkillSource::LocalPath { path: tmp.path().to_path_buf() })
            .await
            .unwrap();
        assert_eq!(pkg.name, "demo");
        assert_eq!(pkg.templates.len(), 1);
        assert_eq!(pkg.templates[0].name, "t.json");
    }

    #[tokio::test]
    async fn github_fetch_round_trip() {
        let api = MockServer::start().await;
        let raw = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/ow/re/commits/HEAD"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"sha":"deadbeef"})),
            )
            .mount(&api)
            .await;

        Mock::given(method("GET"))
            .and(path("/ow/re/deadbeef/skill-a/SKILL.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "---\nname: skill-a\ndescription: d\n---\nbody",
            ))
            .mount(&raw)
            .await;

        // references/ + templates/ → 404
        Mock::given(method("GET"))
            .and(path("/repos/ow/re/contents/skill-a/references"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&api)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/ow/re/contents/skill-a/templates"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&api)
            .await;

        let f = Fetcher::with_bases(api.uri(), raw.uri());
        let pkg = f
            .fetch(&SkillSource::Github {
                owner: "ow".into(),
                repo: "re".into(),
                subpath: "skill-a".into(),
                r#ref: GitRef::Latest,
            })
            .await
            .unwrap();
        assert_eq!(pkg.name, "skill-a");
        assert_eq!(pkg.resolved_sha, "deadbeef");
    }
}
