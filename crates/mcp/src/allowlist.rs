//! Per-channel MCP server allowlists.
//!
//! A coding channel and a regular-chat channel can each have their own list of
//! allowed MCP servers, so a `linear` MCP server enabled for chat doesn't leak
//! into coding mode (and vice versa for `github-code-search`).

use std::collections::{HashMap, HashSet};

/// Per-channel allowlist for MCP servers.
///
/// If a channel is **not** present in the map, all servers are allowed
/// (back-compat default). If a channel **is** present, only the listed
/// servers are allowed; all others are denied.
#[derive(Debug, Clone, Default)]
pub struct McpChannelAllowlist {
    inner: HashMap<String, HashSet<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowDecision {
    Allowed,
    Denied,
}

impl McpChannelAllowlist {
    /// Allow `server` on `channel`.
    pub fn allow(&mut self, channel: &str, server: &str) {
        self.inner
            .entry(channel.to_string())
            .or_default()
            .insert(server.to_string());
    }

    /// Decide whether `server` may run on `channel`.
    ///
    /// - Unconfigured channel → `Allowed` (back-compat).
    /// - Configured channel, server in set → `Allowed`.
    /// - Configured channel, server not in set → `Denied`.
    pub fn decide(&self, channel: &str, server: &str) -> AllowDecision {
        match self.inner.get(channel) {
            None => AllowDecision::Allowed,
            Some(set) if set.contains(server) => AllowDecision::Allowed,
            Some(_) => AllowDecision::Denied,
        }
    }
}

impl From<HashMap<String, Vec<String>>> for McpChannelAllowlist {
    fn from(map: HashMap<String, Vec<String>>) -> Self {
        let inner = map
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect();
        Self { inner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_gates_per_channel() {
        let mut a = McpChannelAllowlist::default();
        a.allow("coding", "github");
        a.allow("chat", "linear");

        assert_eq!(a.decide("coding", "github"), AllowDecision::Allowed);
        assert_eq!(a.decide("coding", "linear"), AllowDecision::Denied);
        assert_eq!(a.decide("chat", "github"), AllowDecision::Denied);
        assert_eq!(a.decide("chat", "linear"), AllowDecision::Allowed);
    }

    #[test]
    fn unconfigured_channel_allows_all_for_back_compat() {
        let a = McpChannelAllowlist::default();
        assert_eq!(a.decide("coding", "github"), AllowDecision::Allowed);
        assert_eq!(a.decide("chat", "anything"), AllowDecision::Allowed);
    }

    #[test]
    fn from_hashmap_converts_correctly() {
        let mut map = HashMap::new();
        map.insert("coding".to_string(), vec!["github".to_string()]);
        let a = McpChannelAllowlist::from(map);
        assert_eq!(a.decide("coding", "github"), AllowDecision::Allowed);
        assert_eq!(a.decide("coding", "linear"), AllowDecision::Denied);
    }
}
