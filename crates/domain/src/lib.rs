//! Domain crate — PARA + OKR types for klyntbot.

pub mod area;
pub mod key_result;
pub mod objective;
pub mod project;

pub use area::{Area, AreaColor, AreaPatch, AreaStatus};
pub use key_result::{KeyResult, KeyResultPatch, KeyResultStatus, TrackingMode};
pub use objective::{Objective, ObjectivePatch, ObjectiveStatus};
pub use project::{Project, ProjectColor, ProjectFilter, ProjectPatch, ProjectStatus};
