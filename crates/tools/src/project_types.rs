//! Project management types
//!
//! This module defines the core data structures for the project system,
//! including projects, statuses, colors, filters, and patches.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A project for grouping and organizing todos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String, // 8-char UUID
    pub name: String,
    pub description: Option<String>,
    pub color: ProjectColor,
    pub tags: Vec<String>,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Project status lifecycle
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

/// Project color for visual organization
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
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

impl Project {
    /// Generate short ID (8 chars) from UUID — delegates to shared helper.
    pub fn generate_id() -> String {
        crate::todo_types::generate_short_id()
    }
}

/// Partial update for existing projects
#[derive(Debug, Clone, Default)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub color: Option<ProjectColor>,
    pub tags: Option<Vec<String>>,
    pub status: Option<ProjectStatus>,
}

/// Filter criteria for listing projects
#[derive(Debug, Clone, Default)]
pub struct ProjectFilter {
    pub status: Option<ProjectStatus>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = Project::generate_id();
        let id2 = Project::generate_id();

        assert_eq!(id1.len(), 8, "ID should be 8 characters");
        assert_eq!(id2.len(), 8, "ID should be 8 characters");
        assert_ne!(id1, id2, "IDs should be unique");
    }

    #[test]
    fn test_generate_id_uniqueness_100() {
        use std::collections::HashSet;
        let mut ids = HashSet::new();

        for _ in 0..100 {
            let id = Project::generate_id();
            assert_eq!(id.len(), 8, "ID should be 8 characters");
            assert!(
                ids.insert(id.clone()),
                "ID {} already exists - not unique!",
                id
            );
        }

        assert_eq!(ids.len(), 100, "Should have 100 unique IDs");
    }

    #[test]
    fn test_project_status_serde() {
        use serde_json;

        // Serialize
        let status = ProjectStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"active\"");

        // Deserialize
        let parsed: ProjectStatus = serde_json::from_str("\"paused\"").unwrap();
        assert_eq!(parsed, ProjectStatus::Paused);
    }

    #[test]
    fn test_project_color_serde() {
        use serde_json;

        // Serialize
        let color = ProjectColor::Blue;
        let json = serde_json::to_string(&color).unwrap();
        assert_eq!(json, "\"blue\"");

        // Deserialize
        let parsed: ProjectColor = serde_json::from_str("\"purple\"").unwrap();
        assert_eq!(parsed, ProjectColor::Purple);
    }

    #[test]
    fn test_project_color_default() {
        let color = ProjectColor::default();
        assert_eq!(color, ProjectColor::Orange);
    }

    #[test]
    fn test_project_filter_default() {
        let filter = ProjectFilter::default();
        assert!(filter.status.is_none());
        assert!(filter.tag.is_none());
        assert!(filter.limit.is_none());
    }

    #[test]
    fn test_project_patch_default() {
        let patch = ProjectPatch::default();
        assert!(patch.name.is_none());
        assert!(patch.description.is_none());
        assert!(patch.color.is_none());
        assert!(patch.tags.is_none());
        assert!(patch.status.is_none());
    }
}
