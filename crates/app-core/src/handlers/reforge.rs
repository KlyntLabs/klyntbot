//! Reforge Cycle handlers — expose reforge state, skill versions, and skill reset.

use cognitive::services::reforge::skill_files::{compute_diff, SkillFileManager};
use desktop_shared::commands::reforge::*;
use desktop_shared::errors::ApiError;
use storage::repos::{ReforgeStateRepo, SkillVersionRepo};
use storage::rows::SkillVersionRow;

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    /// Get the current reforge cycle state (last run time, stats, run count).
    pub async fn reforge_state(&self) -> Result<ReforgeStateResponse, ApiError> {
        let repo = ReforgeStateRepo::new(self.repos.pool().clone());
        let row = repo.get().await.map_err(map_storage_err)?;

        let stats_json = row
            .last_run_stats
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        Ok(ReforgeStateResponse {
            last_run_at: row.last_run_at,
            last_run_stats: stats_json,
            run_count: row.run_count,
        })
    }

    /// List all versions of a skill (newest first), omitting full content.
    pub async fn skill_version_list(
        &self,
        skill_name: &str,
    ) -> Result<Vec<SkillVersionResponse>, ApiError> {
        let repo = SkillVersionRepo::new(self.repos.pool().clone());
        let rows = repo
            .list_versions(skill_name)
            .await
            .map_err(map_storage_err)?;
        Ok(rows.into_iter().map(version_to_response).collect())
    }

    /// Get full detail (including content) for a specific skill version.
    pub async fn skill_version_detail(
        &self,
        skill_name: &str,
        version: i64,
    ) -> Result<Vec<SkillVersionDetailResponse>, ApiError> {
        let repo = SkillVersionRepo::new(self.repos.pool().clone());
        let rows = repo
            .get_version(skill_name, version)
            .await
            .map_err(map_storage_err)?;
        Ok(rows.into_iter().map(version_to_detail_response).collect())
    }

    /// List all distinct skill names that have version history.
    pub async fn skill_names(&self) -> Result<SkillListResponse, ApiError> {
        let repo = SkillVersionRepo::new(self.repos.pool().clone());
        let names = repo.list_skill_names().await.map_err(map_storage_err)?;
        Ok(SkillListResponse { skill_names: names })
    }

    /// Reset a skill file to a previous version by reading that version's content
    /// from DB, writing it to disk, and recording a new version with source="User".
    pub async fn skill_version_reset(
        &self,
        skill_name: &str,
        file_path: &str,
        version: i64,
    ) -> Result<(), ApiError> {
        let repo = SkillVersionRepo::new(self.repos.pool().clone());

        // 1. Find the target version's content.
        let target_rows = repo
            .get_version(skill_name, version)
            .await
            .map_err(map_storage_err)?;
        let target = target_rows
            .iter()
            .find(|r| r.file_path == file_path)
            .ok_or_else(|| {
                ApiError::new(
                    "NOT_FOUND",
                    format!("version {version} of {skill_name}/{file_path} not found"),
                )
            })?;
        let new_content = target.content.clone();

        // 2. Read current content from disk for diff computation.
        let config = self.config.read().await;
        let skills_dir = config.data_dir_path().join("skills");
        drop(config);
        let manager = SkillFileManager::new(skills_dir);

        let current_content = {
            let all = manager.read_all();
            all.get(skill_name)
                .and_then(|files| files.iter().find(|f| f.file_path == file_path))
                .map(|f| f.content.clone())
                .unwrap_or_default()
        };

        // 3. Write the target version's content to disk.
        manager
            .write_file(skill_name, file_path, &new_content)
            .map_err(|e| ApiError::new("IO_ERROR", format!("failed to write skill file: {e}")))?;

        // 4. Compute the next version number and diff.
        let latest = repo
            .latest_version(skill_name, file_path)
            .await
            .map_err(map_storage_err)?;
        let next_version = latest.map(|r| r.version + 1).unwrap_or(1);
        let diff = if current_content.is_empty() {
            None
        } else {
            Some(compute_diff(&current_content, &new_content))
        };

        // 5. Record the new version in the DB.
        let row = SkillVersionRow {
            id: uuid::Uuid::new_v4().to_string(),
            skill_name: skill_name.to_string(),
            version: next_version,
            file_path: file_path.to_string(),
            content: new_content,
            diff,
            source: "User".to_string(),
            reason: Some(format!("Reset to v{version}")),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        repo.insert(&row).await.map_err(map_storage_err)?;

        Ok(())
    }
}

fn version_to_response(row: SkillVersionRow) -> SkillVersionResponse {
    SkillVersionResponse {
        id: row.id,
        skill_name: row.skill_name,
        version: row.version,
        file_path: row.file_path,
        diff: row.diff,
        source: row.source,
        reason: row.reason,
        created_at: row.created_at,
    }
}

fn version_to_detail_response(row: SkillVersionRow) -> SkillVersionDetailResponse {
    SkillVersionDetailResponse {
        id: row.id,
        skill_name: row.skill_name,
        version: row.version,
        file_path: row.file_path,
        content: row.content,
        diff: row.diff,
        source: row.source,
        reason: row.reason,
        created_at: row.created_at,
    }
}
