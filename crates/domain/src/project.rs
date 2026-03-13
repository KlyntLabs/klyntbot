//! Project domain type — belongs to an Area, contains OKR objectives.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub area_id: String, // required FK to areas
    pub name: String,
    pub description: Option<String>,
    pub color: ProjectColor,
    pub tags: Vec<String>,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectColor {
    Red,
    #[default]
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Gray,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_id_generation() {
        let id = Project::generate_id();
        assert_eq!(id.len(), 36); // full UUID
        assert_ne!(id, Project::generate_id());
    }
}
