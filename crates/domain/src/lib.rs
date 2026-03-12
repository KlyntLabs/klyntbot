//! Domain crate — PARA + OKR types for klyntbot.

pub mod area;
pub mod key_result;
pub mod objective;
pub mod project;

pub use area::{Area, AreaColor, AreaStatus};
pub use key_result::{KeyResult, KeyResultStatus, TrackingMode};
pub use objective::{Objective, ObjectiveStatus};
pub use project::{Project, ProjectColor, ProjectStatus};
