//! Focus Sessions — timed system-level actions (DND, silent mode, etc.).

pub mod alarm_bridge;
pub mod bridge;
pub mod duration_parser;
pub mod manager;
pub mod repo;

pub use bridge::{FOCUS_OFF_BYTES, FOCUS_ON_BYTES};
pub use manager::{DndManager, DndScheduler, FocusBridge};
pub use repo::{FocusSession, FocusSessionRepo};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FocusMode {
    Dnd,
}

impl FocusMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dnd => "dnd",
        }
    }
}

impl std::str::FromStr for FocusMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "dnd" => Ok(Self::Dnd),
            other => Err(format!("unknown focus mode: {other}")),
        }
    }
}

use async_trait::async_trait;
use common::Result;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub struct FocusFeature;

#[async_trait]
impl FeaturePackage for FocusFeature {
    fn name(&self) -> &str {
        "focus"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "focus".to_string(),
            version: 1,
            description: "Create focus_sessions table".to_string(),
            sql: include_str!("migrations/001_focus_sessions.sql").to_string(),
        }]
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
