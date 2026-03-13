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

impl ProjectStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
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

impl ProjectColor {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "red" => Some(Self::Red),
            "orange" => Some(Self::Orange),
            "yellow" => Some(Self::Yellow),
            "green" => Some(Self::Green),
            "blue" => Some(Self::Blue),
            "purple" => Some(Self::Purple),
            "gray" | "grey" => Some(Self::Gray),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_status_roundtrip() {
        for (s, expected) in [
            ("active", ProjectStatus::Active),
            ("paused", ProjectStatus::Paused),
            ("completed", ProjectStatus::Completed),
            ("archived", ProjectStatus::Archived),
        ] {
            let parsed = ProjectStatus::from_str_loose(s).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn test_project_color_roundtrip() {
        for (s, expected) in [
            ("red", ProjectColor::Red),
            ("orange", ProjectColor::Orange),
            ("yellow", ProjectColor::Yellow),
            ("green", ProjectColor::Green),
            ("blue", ProjectColor::Blue),
            ("purple", ProjectColor::Purple),
            ("gray", ProjectColor::Gray),
        ] {
            let parsed = ProjectColor::from_str_loose(s).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), s);
        }
    }

    #[test]
    fn test_project_color_grey_alias() {
        // "grey" (UK spelling) should resolve to Gray
        assert_eq!(
            ProjectColor::from_str_loose("grey"),
            Some(ProjectColor::Gray)
        );
        assert_eq!(
            ProjectColor::from_str_loose("GREY"),
            Some(ProjectColor::Gray)
        );
    }

    #[test]
    fn test_project_id_generation() {
        let id = Project::generate_id();
        assert_eq!(id.len(), 36); // full UUID
        assert_ne!(id, Project::generate_id());
    }
}
