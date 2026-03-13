use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::EnergyLevel;

/// In-memory state for an active focus session on a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTaskFocus {
    pub task_id: String,
    pub started_at: DateTime<Utc>,
    pub energy_level: Option<EnergyLevel>,
}
