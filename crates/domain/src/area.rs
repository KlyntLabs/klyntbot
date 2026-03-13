//! Area domain type — top-level PARA container (Work, Personal, etc.).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A PARA area — the highest-level organizational container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: AreaColor,
    pub icon: Option<String>,
    pub position: i32,
    pub status: AreaStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Area {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AreaStatus {
    Active,
    Archived,
}

impl AreaStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

impl std::fmt::Display for AreaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AreaColor {
    #[default]
    Blue,
    Green,
    Purple,
    Orange,
    Red,
    Yellow,
    Gray,
}

impl AreaColor {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "blue" => Some(Self::Blue),
            "green" => Some(Self::Green),
            "purple" => Some(Self::Purple),
            "orange" => Some(Self::Orange),
            "red" => Some(Self::Red),
            "yellow" => Some(Self::Yellow),
            "gray" | "grey" => Some(Self::Gray),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Purple => "purple",
            Self::Orange => "orange",
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Gray => "gray",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_area_id_generation() {
        let id = Area::generate_id();
        assert_eq!(id.len(), 36); // full UUID
        assert_ne!(id, Area::generate_id());
    }

    #[test]
    fn test_area_status_roundtrip() {
        assert_eq!(
            AreaStatus::from_str_loose("active"),
            Some(AreaStatus::Active)
        );
        assert_eq!(
            AreaStatus::from_str_loose("ARCHIVED"),
            Some(AreaStatus::Archived)
        );
        assert_eq!(AreaStatus::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_area_color_roundtrip() {
        for color in [
            AreaColor::Blue,
            AreaColor::Green,
            AreaColor::Purple,
            AreaColor::Orange,
            AreaColor::Red,
            AreaColor::Yellow,
            AreaColor::Gray,
        ] {
            let s = color.as_str();
            assert_eq!(AreaColor::from_str_loose(s), Some(color));
        }
    }
}
