#![cfg(target_os = "linux")]

use crate::policy::SandboxPolicy;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperMode {
    /// Helper runs INSIDE a bwrap namespace; applies Landlock as defense-in-depth then exec.
    WithBwrap,
    /// bwrap absent on host; helper applies Landlock-only then exec. Network not isolated.
    LandlockOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperPolicy {
    pub mode: HelperMode,
    pub sandbox: SandboxPolicy,
}

impl HelperPolicy {
    pub fn to_base64_json(&self) -> Result<String, serde_json::Error> {
        let bytes = serde_json::to_vec(self)?;
        Ok(base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes))
    }

    pub fn from_base64_json(s: &str) -> Result<Self, String> {
        let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(s).map_err(|e| format!("base64: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("json: {e}"))
    }
}
