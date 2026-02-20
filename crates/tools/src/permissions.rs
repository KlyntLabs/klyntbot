//! Tool authorization and permission model.
//!
//! Re-exported from tools_core so that `tools::PermissionLevel == tools_core::PermissionLevel`.
// All tools implementing the Tool trait reference `tools_core::PermissionLevel`, so consumers
// of this module get the same type without duplicate definitions.

pub use tools_core::permissions::{PermissionLevel, ToolPermissions};
