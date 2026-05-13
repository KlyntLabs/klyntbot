//! Safety cap — silent backstop on token/turn exhaustion.
//!
//! Klynt's primary termination model is **model self-stop**: the loop exits
//! when the LLM returns a final response with no tool calls (OpenCode-style).
//! `SafetyCap` is a hard backstop for runaway models — cron jobs and nightly
//! reforge cycles where an unbounded loop would be a real cost concern.
//!
//! Hitting a cap is **not** a graceful path. The loop aborts with
//! `LoopFinishReason::SafetyTurnLimit` or `LoopFinishReason::TokenLimit` and surfaces
//! whatever partial content was accumulated. There is no wrap-up message,
//! no forced synthesis pass, no coercion prompt.

use serde::{Deserialize, Serialize};

// ── Depth Modes ──────────────────────────────────────────────

/// User-selectable depth mode. Controls how deeply the agent thinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepthMode {
    #[default]
    Normal,
    DeepThink,
    Ultra,
}

impl DepthMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::DeepThink => "deep_think",
            Self::Ultra => "ultra",
        }
    }
}

impl std::fmt::Display for DepthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Default safety caps ──────────────────────────────────────

/// Default budget parameters used by HUD and config.
///
/// The main agent runs without a turn cap (`SafetyCap::new` uses `u32::MAX`).
/// Subagents and the coding review pass set their own explicit caps via
/// `SafetyCap::with_limits`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBudget {
    pub normal_tokens: u64,
    pub deep_multiplier: f32,
    pub ultra_multiplier: f32,
}

impl Default for SkillBudget {
    fn default() -> Self {
        Self {
            // 0 = disabled. `SafetyCap::token_cap_hit` guards on `max_tokens > 0`,
            // so the token cap never fires for interactive chat.
            normal_tokens: 0,
            deep_multiplier: 1.5,
            ultra_multiplier: 3.0,
        }
    }
}

// ── Safety cap ───────────────────────────────────────────────

/// Hard upper bound on tokens and turns for a single execute_loop run.
#[derive(Debug, Clone)]
pub struct SafetyCap {
    pub depth: DepthMode,
    max_tokens: u64,
    max_turns: u32,
    tokens_used: u64,
    turns_used: u32,
}

impl SafetyCap {
    /// Create a cap from the user's depth choice.
    ///
    /// The main agent has no turn cap — `max_turns` is always `u32::MAX`.
    /// Depth only affects the token cap (which is disabled by default).
    /// Subagents and the coding review pass set explicit caps via
    /// [`SafetyCap::with_limits`].
    pub fn new(depth: DepthMode) -> Self {
        let base = SkillBudget::default();
        let max_tokens = match depth {
            DepthMode::Normal => base.normal_tokens,
            DepthMode::DeepThink => {
                (base.normal_tokens as f64 * base.deep_multiplier as f64) as u64
            }
            DepthMode::Ultra => (base.normal_tokens as f64 * base.ultra_multiplier as f64) as u64,
        };
        Self {
            depth,
            max_tokens,
            max_turns: u32::MAX,
            tokens_used: 0,
            turns_used: 0,
        }
    }

    /// Create a cap with explicit limits (for subagents, autotuner, tests).
    pub fn with_limits(depth: DepthMode, max_tokens: u64, max_turns: u32) -> Self {
        Self {
            depth,
            max_tokens,
            max_turns,
            tokens_used: 0,
            turns_used: 0,
        }
    }

    pub fn deduct(&mut self, usage: &providers::Usage) {
        self.tokens_used += usage.prompt_tokens as u64 + usage.completion_tokens as u64;
    }

    pub fn tick_turn(&mut self) {
        self.turns_used += 1;
    }

    /// True when either cap has been hit (no synthesis reserve).
    pub fn turn_cap_hit(&self) -> bool {
        self.turns_used >= self.max_turns
    }

    pub fn token_cap_hit(&self) -> bool {
        self.max_tokens > 0 && self.tokens_used >= self.max_tokens
    }

    pub fn tokens_used(&self) -> u64 {
        self.tokens_used
    }

    pub fn turns_used(&self) -> u32 {
        self.turns_used
    }

    pub fn max_turns(&self) -> u32 {
        self.max_turns
    }

    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_uses_defaults() {
        let mut cap = SafetyCap::new(DepthMode::Normal);
        assert_eq!(cap.max_tokens, 0, "token cap disabled by default");
        assert_eq!(cap.max_turns, u32::MAX, "main agent has no turn cap");
        assert!(!cap.turn_cap_hit());
        assert!(!cap.token_cap_hit());
        // A massive usage still must not trip the token cap when disabled.
        cap.deduct(&providers::Usage {
            prompt_tokens: 10_000_000,
            completion_tokens: 10_000_000,
            ..Default::default()
        });
        assert!(!cap.token_cap_hit());
    }

    #[test]
    fn deep_think_keeps_caps_disabled() {
        let cap = SafetyCap::new(DepthMode::DeepThink);
        // Token cap stays disabled (multiplier × 0 = 0); turn cap is unbounded.
        assert_eq!(cap.max_tokens, 0);
        assert_eq!(cap.max_turns, u32::MAX);
    }

    #[test]
    fn ultra_unlimited_turns() {
        let cap = SafetyCap::new(DepthMode::Ultra);
        assert_eq!(cap.max_turns, u32::MAX);
        assert_eq!(cap.max_tokens, 0);
    }

    #[test]
    fn turn_cap_hit_at_limit() {
        let mut cap = SafetyCap::with_limits(DepthMode::Normal, 1_000_000, 3);
        cap.tick_turn();
        cap.tick_turn();
        assert!(!cap.turn_cap_hit());
        cap.tick_turn();
        assert!(cap.turn_cap_hit());
    }

    #[test]
    fn token_cap_hit_at_limit() {
        let mut cap = SafetyCap::with_limits(DepthMode::Normal, 10_000, 100);
        let usage = providers::Usage {
            prompt_tokens: 6000,
            completion_tokens: 3000,
            ..Default::default()
        };
        cap.deduct(&usage);
        assert!(!cap.token_cap_hit());
        cap.deduct(&usage);
        assert!(cap.token_cap_hit());
    }

    #[test]
    fn no_synthesis_reserve_subtraction() {
        // Regression: the old budget reserved 2000 tokens for synthesis,
        // exhausting early. SafetyCap exhausts at the literal max.
        let mut cap = SafetyCap::with_limits(DepthMode::Normal, 10_000, 100);
        let usage = providers::Usage {
            prompt_tokens: 5000,
            completion_tokens: 4000,
            ..Default::default()
        };
        cap.deduct(&usage);
        assert!(!cap.token_cap_hit(), "9000 < 10_000 — must not be capped");
    }
}
