use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableVersion {
    pub sha: String,
    pub tag: Option<String>,
    pub message: String,
    pub date: String,
}

pub struct UpdatesFetcher {
    http: reqwest::Client,
    api_base: String,
}

impl UpdatesFetcher {
    pub fn new(api_base: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("klyntbot-skills-registry")
                .build()
                .expect("reqwest"),
            api_base,
        }
    }

    /// List commits on `owner/repo` touching `subpath` since `installed_sha` (exclusive).
    pub async fn list_newer(
        &self,
        owner: &str,
        repo: &str,
        subpath: &str,
        installed_sha: &str,
    ) -> Result<Vec<AvailableVersion>> {
        let url = format!(
            "{}/repos/{}/{}/commits?path={}&per_page=50",
            self.api_base, owner, repo, subpath
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| KlyntbotError::Storage(format!("commits GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(KlyntbotError::Storage(format!(
                "commits: HTTP {}",
                resp.status()
            )));
        }
        let items: Vec<Value> = resp
            .json()
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for item in items {
            let sha = item
                .get("sha")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if sha == installed_sha {
                break;
            }
            let message = item
                .pointer("/commit/message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let date = item
                .pointer("/commit/author/date")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            out.push(AvailableVersion {
                sha,
                tag: None,
                message,
                date,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn lists_newer_commits_until_installed() {
        let api = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/repos/ow/re/commits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "sha": "aaa", "commit": { "message": "fix", "author": { "date": "2026-04-14T00:00:00Z" } } },
                { "sha": "bbb", "commit": { "message": "feat", "author": { "date": "2026-04-13T00:00:00Z" } } },
                { "sha": "ccc", "commit": { "message": "old", "author": { "date": "2026-04-10T00:00:00Z" } } }
            ])))
            .mount(&api)
            .await;

        let uf = UpdatesFetcher::new(api.uri());
        let out = uf.list_newer("ow", "re", "skill-a", "ccc").await.unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].sha, "aaa");
    }
}
