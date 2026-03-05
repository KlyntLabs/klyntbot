use sqlx::SqlitePool;
use tracing::warn;

use crate::types::{ActivityCategory, CategoryRules, CategoryType};

#[derive(sqlx::FromRow)]
struct CategoryRow {
    id: String,
    name: String,
    category_type: String,
    color: Option<String>,
    icon: Option<String>,
    rules: Option<String>,
    is_system: bool,
}

impl From<CategoryRow> for ActivityCategory {
    fn from(row: CategoryRow) -> Self {
        let rules = row
            .rules
            .as_deref()
            .and_then(|r| serde_json::from_str::<CategoryRules>(r).ok());
        let category_type = row.category_type.parse::<CategoryType>().unwrap_or_else(|_| {
            warn!(raw = %row.category_type, "unknown category_type in DB, defaulting to Neutral");
            CategoryType::Neutral
        });
        Self {
            id: row.id,
            name: row.name,
            category_type,
            color: row.color,
            icon: row.icon,
            rules,
            is_system: row.is_system,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivityCategoryRepo {
    pool: SqlitePool,
}

impl ActivityCategoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, id: &str) -> common::Result<Option<ActivityCategory>> {
        let row = sqlx::query_as::<_, CategoryRow>(
            "SELECT id, name, category_type, color, icon, rules, is_system FROM activity_categories WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(ActivityCategory::from))
    }

    pub async fn list_all(&self) -> common::Result<Vec<ActivityCategory>> {
        let rows = sqlx::query_as::<_, CategoryRow>(
            "SELECT id, name, category_type, color, icon, rules, is_system FROM activity_categories ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(ActivityCategory::from).collect())
    }

    pub async fn upsert(&self, cat: &ActivityCategory) -> common::Result<()> {
        let rules_json = cat
            .rules
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let cat_type = cat.category_type.to_string();
        sqlx::query(
            r#"INSERT INTO activity_categories (id, name, category_type, color, icon, rules, is_system)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   category_type = excluded.category_type,
                   color = excluded.color,
                   icon = excluded.icon,
                   rules = excluded.rules"#,
        )
        .bind(&cat.id)
        .bind(&cat.name)
        .bind(&cat_type)
        .bind(&cat.color)
        .bind(&cat.icon)
        .bind(&rules_json)
        .bind(cat.is_system)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> common::Result<bool> {
        let result = sqlx::query("DELETE FROM activity_categories WHERE id = ?1 AND is_system = FALSE")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected() > 0)
    }
}
