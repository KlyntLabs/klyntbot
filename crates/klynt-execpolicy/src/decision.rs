use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    /// Codex calls this `Prompt`; klyntbot calls it `Ask` (matches what
    /// the chat-inline approval card surfaces to users).
    Ask,
    /// Codex calls this `Forbidden`; klyntbot uses `Forbid` for brevity.
    Forbid,
    /// Klynt-specific: signals "no rule matched"; falls through to the
    /// unified approval gate.
    FallThrough,
}

impl Decision {
    /// Map from the Starlark `decision="..."` string parameter on `prefix_rule`.
    /// Codex parser uses `"allow" | "prompt" | "forbidden"`. Klynt's `Ask`
    /// alias accepts both `"ask"` and `"prompt"`.
    pub fn from_starlark_str(s: &str) -> Option<Self> {
        match s {
            "allow" => Some(Self::Allow),
            "ask" | "prompt" => Some(Self::Ask),
            "forbid" | "forbidden" => Some(Self::Forbid),
            _ => None,
        }
    }
}
