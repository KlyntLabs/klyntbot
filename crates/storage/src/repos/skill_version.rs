//! Repository for the `skill_versions` table — tracks skill file version history.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::SkillVersionRow;

#[derive(Debug, Clone)]
pub struct SkillVersionRepo {
    pool: SqlitePool,
}

impl SkillVersionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &SkillVersionRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO skill_versions (id, skill_name, version, file_path, content, diff, source, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&row.id)
        .bind(&row.skill_name)
        .bind(row.version)
        .bind(&row.file_path)
        .bind(&row.content)
        .bind(&row.diff)
        .bind(&row.source)
        .bind(&row.reason)
        .bind(&row.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn latest_version(
        &self,
        skill_name: &str,
        file_path: &str,
    ) -> Result<Option<SkillVersionRow>, StorageError> {
        let row = sqlx::query_as::<_, SkillVersionRow>(
            "SELECT id, skill_name, version, file_path, content, diff, source, reason, created_at
             FROM skill_versions
             WHERE skill_name = ?1 AND file_path = ?2
             ORDER BY version DESC LIMIT 1",
        )
        .bind(skill_name)
        .bind(file_path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_versions(&self, skill_name: &str) -> Result<Vec<SkillVersionRow>, StorageError> {
        let rows = sqlx::query_as::<_, SkillVersionRow>(
            "SELECT id, skill_name, version, file_path, content, diff, source, reason, created_at
             FROM skill_versions
             WHERE skill_name = ?1
             ORDER BY version DESC",
        )
        .bind(skill_name)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_version(
        &self,
        skill_name: &str,
        version: i64,
    ) -> Result<Vec<SkillVersionRow>, StorageError> {
        let rows = sqlx::query_as::<_, SkillVersionRow>(
            "SELECT id, skill_name, version, file_path, content, diff, source, reason, created_at
             FROM skill_versions
             WHERE skill_name = ?1 AND version = ?2",
        )
        .bind(skill_name)
        .bind(version)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_skill_names(&self) -> Result<Vec<String>, StorageError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT skill_name FROM skill_versions ORDER BY skill_name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    fn test_version(skill: &str, version: i64, source: &str) -> SkillVersionRow {
        SkillVersionRow {
            id: uuid::Uuid::new_v4().to_string(),
            skill_name: skill.to_string(),
            version,
            file_path: "SKILL.md".to_string(),
            content: format!("content v{version}"),
            diff: Some(format!("diff v{version}")),
            source: source.to_string(),
            reason: Some(format!("reason v{version}")),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_latest() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());
        repo.insert(&test_version("general", 1, "Seed")).await.unwrap();
        repo.insert(&test_version("general", 2, "Reforge")).await.unwrap();
        let latest = repo.latest_version("general", "SKILL.md").await.unwrap().unwrap();
        assert_eq!(latest.version, 2);
        assert_eq!(latest.source, "Reforge");
    }

    #[tokio::test]
    async fn test_list_versions() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());
        repo.insert(&test_version("general", 1, "Seed")).await.unwrap();
        repo.insert(&test_version("general", 2, "Reforge")).await.unwrap();
        repo.insert(&test_version("general", 3, "User")).await.unwrap();
        let versions = repo.list_versions("general").await.unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, 3); // DESC order
    }

    #[tokio::test]
    async fn test_latest_version_returns_none_for_unknown() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());
        let latest = repo.latest_version("nonexistent", "SKILL.md").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_list_skill_names() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SkillVersionRepo::new(pool.inner().clone());
        repo.insert(&test_version("general", 1, "Seed")).await.unwrap();
        repo.insert(&test_version("finance", 1, "Seed")).await.unwrap();
        let names = repo.list_skill_names().await.unwrap();
        assert_eq!(names, vec!["finance", "general"]);
    }
}
