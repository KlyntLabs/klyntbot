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

// ---------------------------------------------------------------------------
// Row ↔ domain conversions (storage layer)
// ---------------------------------------------------------------------------

impl ProjectStatus {
    fn from_str_loose(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "archived" => Self::Archived,
            _ => Self::Active,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }
}

impl ProjectColor {
    fn from_str_loose(s: &str) -> Self {
        match s {
            "red" => Self::Red,
            "orange" => Self::Orange,
            "yellow" => Self::Yellow,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "purple" => Self::Purple,
            "gray" => Self::Gray,
            _ => Self::default(),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Gray => "gray",
        }
    }
}

impl From<storage::ProjectRow> for Project {
    fn from(row: storage::ProjectRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            color: ProjectColor::from_str_loose(&row.color),
            tags: row.tags,
            status: ProjectStatus::from_str_loose(&row.status),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&Project> for storage::ProjectRow {
    fn from(project: &Project) -> Self {
        Self {
            id: project.id.clone(),
            name: project.name.clone(),
            description: project.description.clone(),
            color: project.color.as_str().to_string(),
            tags: project.tags.clone(),
            status: project.status.as_str().to_string(),
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

/// Convert a domain `ProjectPatch` into a storage `ProjectPatch`.
impl ProjectPatch {
    pub fn to_storage_patch(&self, id: &str) -> storage::ProjectPatch {
        storage::ProjectPatch {
            id: id.to_string(),
            name: self.name.clone(),
            description: self.description.clone(),
            color: self.color.map(|c| c.as_str().to_string()),
            tags: self.tags.clone(),
            status: self.status.map(|s| s.as_str().to_string()),
        }
    }
}

/// Convert a domain `ProjectFilter` into a storage `ProjectFilter`.
impl ProjectFilter {
    pub fn to_storage_filter(&self) -> storage::ProjectFilter {
        storage::ProjectFilter {
            status: self.status.map(|s| s.as_str().to_string()),
            tags: self.tag.as_ref().map(|t| vec![t.clone()]),
            limit: self.limit.map(|l| l as i64),
        }
    }
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
}
