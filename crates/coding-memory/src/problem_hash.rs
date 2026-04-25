//! Canonical hash of a bug-problem statement.
//!
//! Same logical problem phrased differently must collide. `FixAttempt`
//! clustering (repeat-attempt detection, counterfactual matching) uses this.

use serde::{Deserialize, Serialize};

/// 16-hex-char prefix of a `blake3` hash over a canonicalized problem string.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ProblemHash(String);

impl ProblemHash {
    /// Hash a problem statement with canonicalization (lowercase, collapse whitespace).
    #[must_use]
    pub fn of(raw: &str) -> Self {
        let canon = canonicalize(raw);
        let h = blake3::hash(canon.as_bytes());
        let hex = h.to_hex();
        Self(hex.as_str()[..16].to_string())
    }

    /// Accessor for the 16-char hex string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct from an already-computed value (e.g. DB load).
    #[must_use]
    pub fn from_raw(s: String) -> Self {
        Self(s)
    }
}

fn canonicalize(raw: &str) -> String {
    let lower = raw.to_lowercase();
    lower.split_whitespace().collect::<Vec<_>>().join(" ")
}
