//! skills-marketplace: install/upgrade/uninstall third-party skills with versioning.

pub mod repo;
pub mod types;

pub use repo::{AdaptedSkillsRepo, InstalledSkillsRepo};
pub use types::*;

use tools_core::FeatureMigration;

pub struct SkillsMarketplaceFeature;

impl SkillsMarketplaceFeature {
    pub fn migrations() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "skills_marketplace".to_string(),
            version: 1,
            description: "installed_skills + adapted_skills tables".to_string(),
            sql: include_str!("../migrations/001_skills_marketplace.sql").to_string(),
        }]
    }
}

#[cfg(test)]
pub mod test_helpers {
    pub async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let sql = include_str!("../migrations/001_skills_marketplace.sql");
        for stmt in sql.split(';') {
            let t = stmt.trim();
            if !t.is_empty() {
                sqlx::query(t).execute(pool.inner()).await.unwrap();
            }
        }
        pool.inner().clone()
    }
}
