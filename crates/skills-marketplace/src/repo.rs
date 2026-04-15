use chrono::Utc;
use common::{KlyntbotError, Result};
use sqlx::{FromRow, SqlitePool};

use crate::types::{AdaptedSkillRow, InstalledSkill, SourceType};

fn map_err<E: std::fmt::Display>(e: E) -> KlyntbotError {
    KlyntbotError::Storage(e.to_string())
}

#[derive(FromRow)]
struct InstalledRow {
    name: String,
    source_type: String,
    source_ref: String,
    installed_version: String,
    installed_sha: String,
    enabled: i64,
    is_adapted: i64,
    bootstrapped_databases: Option<String>,
    installed_at: String,
    updated_at: String,
}

impl TryFrom<InstalledRow> for InstalledSkill {
    type Error = KlyntbotError;
    fn try_from(r: InstalledRow) -> Result<Self> {
        let source_type = match r.source_type.as_str() {
            "github" => SourceType::Github,
            "skills_sh" => SourceType::SkillsSh,
            "local" => SourceType::Local,
            "bundled" => SourceType::Bundled,
            other => {
                return Err(KlyntbotError::Storage(format!(
                    "unknown source_type {other}"
                )))
            }
        };
        let bootstrapped: Vec<String> = r
            .bootstrapped_databases
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| serde_json::from_str(s))
            .transpose()
            .map_err(map_err)?
            .unwrap_or_default();
        Ok(InstalledSkill {
            name: r.name,
            source_type,
            source_ref: r.source_ref,
            installed_version: r.installed_version,
            installed_sha: r.installed_sha,
            enabled: r.enabled != 0,
            is_adapted: r.is_adapted != 0,
            bootstrapped_databases: bootstrapped,
            installed_at: r.installed_at,
            updated_at: r.updated_at,
        })
    }
}

pub struct InstalledSkillsRepo {
    pool: SqlitePool,
}

impl InstalledSkillsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, s: &InstalledSkill) -> Result<()> {
        let boots = serde_json::to_string(&s.bootstrapped_databases).map_err(map_err)?;
        sqlx::query(
            "INSERT INTO installed_skills \
             (name, source_type, source_ref, installed_version, installed_sha, enabled, is_adapted, \
              bootstrapped_databases, installed_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&s.name)
        .bind(s.source_type.as_str())
        .bind(&s.source_ref)
        .bind(&s.installed_version)
        .bind(&s.installed_sha)
        .bind(s.enabled as i64)
        .bind(s.is_adapted as i64)
        .bind(&boots)
        .bind(&s.installed_at)
        .bind(&s.updated_at)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(())
    }

    pub async fn get(&self, name: &str) -> Result<Option<InstalledSkill>> {
        let row: Option<InstalledRow> =
            sqlx::query_as("SELECT * FROM installed_skills WHERE name = ?")
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_err)?;
        row.map(InstalledSkill::try_from).transpose()
    }

    pub async fn list(&self) -> Result<Vec<InstalledSkill>> {
        let rows: Vec<InstalledRow> =
            sqlx::query_as("SELECT * FROM installed_skills ORDER BY name ASC")
                .fetch_all(&self.pool)
                .await
                .map_err(map_err)?;
        rows.into_iter().map(InstalledSkill::try_from).collect()
    }

    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE installed_skills SET enabled = ?, updated_at = ? WHERE name = ?")
            .bind(enabled as i64)
            .bind(&now)
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    pub async fn delete(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM installed_skills WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    pub async fn update_version(
        &self,
        name: &str,
        version: &str,
        sha: &str,
        bootstrapped: &[String],
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let boots = serde_json::to_string(bootstrapped).map_err(map_err)?;
        sqlx::query(
            "UPDATE installed_skills SET installed_version = ?, installed_sha = ?, \
             bootstrapped_databases = ?, updated_at = ? WHERE name = ?",
        )
        .bind(version)
        .bind(sha)
        .bind(&boots)
        .bind(&now)
        .bind(name)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(())
    }
}

pub struct AdaptedSkillsRepo {
    pool: SqlitePool,
}

impl AdaptedSkillsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, cache_key: &str) -> Result<Option<AdaptedSkillRow>> {
        let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT cache_key, adapted_skill_md, generated_templates, rationale, adapter_model, created_at \
             FROM adapted_skills WHERE cache_key = ?",
        )
        .bind(cache_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(match row {
            Some((k, md, tpls, r, model, at)) => Some(AdaptedSkillRow {
                cache_key: k,
                adapted_skill_md: md,
                generated_templates: serde_json::from_str(&tpls).map_err(map_err)?,
                rationale: r,
                adapter_model: model,
                created_at: at,
            }),
            None => None,
        })
    }

    pub async fn upsert(
        &self,
        cache_key: &str,
        skill_md: &str,
        templates: &serde_json::Value,
        rationale: &str,
        model: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let tpls = serde_json::to_string(templates).map_err(map_err)?;
        sqlx::query(
            "INSERT INTO adapted_skills (cache_key, adapted_skill_md, generated_templates, rationale, adapter_model, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(cache_key) DO UPDATE SET \
               adapted_skill_md = excluded.adapted_skill_md, \
               generated_templates = excluded.generated_templates, \
               rationale = excluded.rationale, \
               adapter_model = excluded.adapter_model, \
               created_at = excluded.created_at",
        )
        .bind(cache_key)
        .bind(skill_md)
        .bind(&tpls)
        .bind(rationale)
        .bind(model)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_and_roundtrip() {
        let pool = crate::test_helpers::setup_pool().await;
        let repo = InstalledSkillsRepo::new(pool);
        let skill = InstalledSkill {
            name: "reading-list".into(),
            source_type: SourceType::Github,
            source_ref: "owner/repo/reading-list".into(),
            installed_version: "1.0.3".into(),
            installed_sha: "abc123".into(),
            enabled: true,
            is_adapted: false,
            bootstrapped_databases: vec!["db1".into(), "db2".into()],
            installed_at: "2026-04-14T00:00:00Z".into(),
            updated_at: "2026-04-14T00:00:00Z".into(),
        };
        repo.insert(&skill).await.unwrap();

        let fetched = repo.get("reading-list").await.unwrap().unwrap();
        assert_eq!(fetched.name, "reading-list");
        assert_eq!(fetched.bootstrapped_databases, vec!["db1", "db2"]);
    }

    #[tokio::test]
    async fn update_version_and_list() {
        let pool = crate::test_helpers::setup_pool().await;
        let repo = InstalledSkillsRepo::new(pool);
        let skill = InstalledSkill {
            name: "x".into(),
            source_type: SourceType::Github,
            source_ref: "o/r/x".into(),
            installed_version: "1.0.0".into(),
            installed_sha: "aaa".into(),
            enabled: true,
            is_adapted: false,
            bootstrapped_databases: vec![],
            installed_at: "t".into(),
            updated_at: "t".into(),
        };
        repo.insert(&skill).await.unwrap();
        repo.update_version("x", "1.0.1", "bbb", &["dbA".into()])
            .await
            .unwrap();
        let fetched = repo.get("x").await.unwrap().unwrap();
        assert_eq!(fetched.installed_version, "1.0.1");
        assert_eq!(fetched.installed_sha, "bbb");
        assert_eq!(fetched.bootstrapped_databases, vec!["dbA"]);
        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn adapted_cache_upsert() {
        use serde_json::json;
        let pool = crate::test_helpers::setup_pool().await;
        let repo = AdaptedSkillsRepo::new(pool);
        repo.upsert(
            "k1",
            "---\nname: x\n---\nbody",
            &json!([{"name":"t","manifest_json":{}}]),
            "why",
            "claude-opus-4-6",
        )
        .await
        .unwrap();
        let row = repo.get("k1").await.unwrap().unwrap();
        assert_eq!(row.adapter_model, "claude-opus-4-6");
    }
}
