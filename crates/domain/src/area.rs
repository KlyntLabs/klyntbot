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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_area_id_generation() {
        let id = Area::generate_id();
        assert_eq!(id.len(), 36); // full UUID
        assert_ne!(id, Area::generate_id());
    }
}
