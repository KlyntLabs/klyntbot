use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SkillVersionRow {
    pub id: String,
    pub skill_name: String,
    pub version: i64,
    pub file_path: String,
    pub content: String,
    pub diff: Option<String>,
    pub source: String,
    pub reason: Option<String>,
    pub created_at: String,
}
