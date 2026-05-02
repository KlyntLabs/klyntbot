//! Tool authorization and permission model.
//!
//! Provides per-channel permission levels for controlling tool access.
//! When no permissions are configured, all tools are allowed (backward-compatible).

use std::collections::HashMap;

/// Permission level required to execute a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionLevel {
    /// Read-only tools (e.g., read_file, list_dir, web_search)
    ReadOnly = 0,
    /// Standard tools (e.g., todo, project, memory)
    Standard = 1,
    /// Elevated tools (e.g., exec, write_file, edit_file)
    Elevated = 2,
    /// Admin tools (e.g., spawn)
    Admin = 3,
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "ReadOnly"),
            Self::Standard => write!(f, "Standard"),
            Self::Elevated => write!(f, "Elevated"),
            Self::Admin => write!(f, "Admin"),
        }
    }
}

/// Per-channel permission configuration for tool access control.
#[derive(Debug, Clone)]
pub struct ToolPermissions {
    /// Permission level per channel name.
    channel_levels: HashMap<String, PermissionLevel>,
    /// Default permission level for unconfigured channels.
    default_level: PermissionLevel,
}

impl ToolPermissions {
    /// Create a new permission set with a default level.
    pub fn new(default_level: PermissionLevel) -> Self {
        Self {
            channel_levels: HashMap::new(),
            default_level,
        }
    }

    /// Set the permission level for a specific channel.
    pub fn set_channel_level(&mut self, channel: impl Into<String>, level: PermissionLevel) {
        self.channel_levels.insert(channel.into(), level);
    }

    /// Check if a channel has sufficient permission for the required level.
    pub fn is_allowed(&self, channel: &str, required: PermissionLevel) -> bool {
        let granted = self
            .channel_levels
            .get(channel)
            .copied()
            .unwrap_or(self.default_level);
        granted >= required
    }
}
