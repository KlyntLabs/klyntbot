//! Execution budget — token/turn budget with user-controlled depth modes.
//!
//! Replaces the old wall-clock `pipeline_timeout_secs` with a budget model
//! inspired by Claude Code. The budget correlates with work done, not
//! arbitrary time — a slow provider and a fast provider doing the same
//! work use the same budget.

use serde::{Deserialize, Serialize};

// ── Depth Modes ──────────────────────────────────────────────

/// User-selectable depth mode. Controls how deeply the agent thinks.
///
/// - `Normal`: fast, frictionless — default for daily use
/// - `DeepThink`: +Mirror context, coaching evaluation, visible HUD
/// - `Ultra`: full cognitive partner — auto-save, FSRS atoms, visible enrichment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepthMode {
    #[default]
    Normal,
    DeepThink,
    Ultra,
}

impl std::fmt::Display for DepthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::DeepThink => write!(f, "deep_think"),
            Self::Ultra => write!(f, "ultra"),
        }
    }
}

// ── Skill Budget Defaults ────────────────────────────────────

/// Default budget parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBudget {
    pub normal_tokens: u64,
    pub normal_turns: u32,
    pub deep_multiplier: f32,
    pub ultra_multiplier: f32,
}

impl Default for SkillBudget {
    fn default() -> Self {
        Self {
            normal_tokens: 60_000,
            normal_turns: 15,
            deep_multiplier: 1.5,
            ultra_multiplier: 3.0,
        }
    }
}

// ── Execution Budget ─────────────────────────────────────────

/// Minimum tokens reserved for the final synthesis response.
const RESERVED_SYNTHESIS_TOKENS: u64 = 2_000;

/// Percentage of budget at which "wrap up" instruction is injected.
const WRAP_UP_THRESHOLD: f32 = 0.85;

/// Budget state for a single request execution.
///
/// Created at the start of Phase 2, checked before every LLM call in Phase 3,
/// deducted after every response. When the budget is nearly exhausted, a
/// "wrap up" system instruction is injected. When fully exhausted, the loop
/// forces a synthesis response with partial results.
#[derive(Debug, Clone)]
pub struct ExecutionBudget {
    pub depth: DepthMode,
    max_tokens: u64,
    max_turns: u32,
    tokens_used: u64,
    turns_used: u32,
    cost_usd: f64,
}

impl ExecutionBudget {
    /// Create a budget from the user's depth choice.
    pub fn new(depth: DepthMode) -> Self {
        let base = SkillBudget::default();
        let (max_tokens, max_turns) = match depth {
            DepthMode::Normal => (base.normal_tokens, base.normal_turns),
            DepthMode::DeepThink => (
                (base.normal_tokens as f64 * base.deep_multiplier as f64) as u64,
                (base.normal_turns as f64 * base.deep_multiplier as f64) as u32,
            ),
            DepthMode::Ultra => (
                (base.normal_tokens as f64 * base.ultra_multiplier as f64) as u64,
                u32::MAX, // Ultra: unlimited turns, bounded by monthly budget only
            ),
        };

        Self {
            depth,
            max_tokens,
            max_turns,
            tokens_used: 0,
            turns_used: 0,
            cost_usd: 0.0,
        }
    }

    /// Create a budget with explicit limits (for testing / simulator).
    pub fn with_limits(depth: DepthMode, max_tokens: u64, max_turns: u32) -> Self {
        Self {
            depth,
            max_tokens,
            max_turns,
            tokens_used: 0,
            turns_used: 0,
            cost_usd: 0.0,
        }
    }

    pub fn deduct(&mut self, usage: &providers::Usage) {
        let total = usage.prompt_tokens as u64 + usage.completion_tokens as u64;
        self.tokens_used += total;
    }

    /// Record estimated cost for this response.
    pub fn record_cost(&mut self, cost_usd: f64) {
        self.cost_usd += cost_usd;
    }

    pub fn tick_turn(&mut self) {
        self.turns_used += 1;
    }

    /// True when the "wrap up soon" instruction should be injected.
    pub fn should_wrap_up(&self) -> bool {
        self.remaining_pct() < (1.0 - WRAP_UP_THRESHOLD)
    }

    /// True when the budget is fully exhausted.
    pub fn exhausted(&self) -> bool {
        self.tokens_used + RESERVED_SYNTHESIS_TOKENS >= self.max_tokens
            || self.turns_used >= self.max_turns
    }

    /// Fraction of budget remaining (0.0–1.0). Uses the tighter of token or turn budget.
    pub fn remaining_pct(&self) -> f32 {
        let token_pct = if self.max_tokens == 0 {
            0.0
        } else {
            1.0 - (self.tokens_used as f32 / self.max_tokens as f32)
        };
        let turn_pct = if self.max_turns == u32::MAX {
            1.0 // Ultra: unlimited turns
        } else if self.max_turns == 0 {
            0.0
        } else {
            1.0 - (self.turns_used as f32 / self.max_turns as f32)
        };
        token_pct.min(turn_pct).clamp(0.0, 1.0)
    }

    /// Extend the budget by additional turns (user tapped "Extend" in HUD).
    pub fn extend_turns(&mut self, additional: u32) {
        if self.max_turns != u32::MAX {
            self.max_turns = self.max_turns.saturating_add(additional);
        }
    }

    // ── Accessors for HUD events ──

    pub fn tokens_used(&self) -> u64 {
        self.tokens_used
    }

    pub fn turns_used(&self) -> u32 {
        self.turns_used
    }

    pub fn cost_usd(&self) -> f64 {
        self.cost_usd
    }

    pub fn max_turns(&self) -> u32 {
        self.max_turns
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_budget_uses_defaults() {
        let budget = ExecutionBudget::new(DepthMode::Normal);
        assert_eq!(budget.max_tokens, 60_000);
        assert_eq!(budget.max_turns, 15);
        assert!(!budget.exhausted());
        assert!(!budget.should_wrap_up());
        assert!((budget.remaining_pct() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn deep_think_scales_by_multiplier() {
        let budget = ExecutionBudget::new(DepthMode::DeepThink);
        assert_eq!(budget.max_tokens, 90_000); // 60K * 1.5
        assert_eq!(budget.max_turns, 22); // 15 * 1.5 = 22.5 → 22 (truncated)
    }

    #[test]
    fn ultra_has_unlimited_turns() {
        let budget = ExecutionBudget::new(DepthMode::Ultra);
        assert_eq!(budget.max_turns, u32::MAX);
        // Token budget is still finite
        assert_eq!(budget.max_tokens, 180_000); // 60K * 3.0
    }

    #[test]
    fn deduct_tracks_usage() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 10_000, 5);
        let usage = providers::Usage {
            prompt_tokens: 3000,
            completion_tokens: 1000,
            ..Default::default()
        };
        budget.deduct(&usage);
        budget.tick_turn();

        assert_eq!(budget.tokens_used(), 4000);
        assert_eq!(budget.turns_used(), 1);
    }

    #[test]
    fn exhausted_by_tokens() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 10_000, 100);
        let usage = providers::Usage {
            prompt_tokens: 2000,
            completion_tokens: 1500,
            ..Default::default()
        };
        budget.deduct(&usage);
        assert!(!budget.exhausted()); // 3500 used + 2000 reserved = 5500 < 10000

        budget.deduct(&usage); // 7000 used
        assert!(!budget.exhausted()); // 7000 + 2000 = 9000 < 10000

        budget.deduct(&usage); // 10500 used → exceeds 10000
        assert!(budget.exhausted());
    }

    #[test]
    fn exhausted_by_turns() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 1_000_000, 3);
        budget.tick_turn();
        budget.tick_turn();
        assert!(!budget.exhausted());
        budget.tick_turn();
        assert!(budget.exhausted());
    }

    #[test]
    fn wrap_up_at_85_percent() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 10_000, 100);
        assert!(!budget.should_wrap_up());

        // Use 8600 tokens → 86% → should wrap up
        let usage = providers::Usage {
            prompt_tokens: 5000,
            completion_tokens: 3600,
            ..Default::default()
        };
        budget.deduct(&usage);
        assert!(budget.should_wrap_up());
    }

    #[test]
    fn extend_turns_adds_to_budget() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 100_000, 10);
        for _ in 0..10 {
            budget.tick_turn();
        }
        assert!(budget.exhausted());

        budget.extend_turns(20);
        assert!(!budget.exhausted());
        assert_eq!(budget.max_turns(), 30);
    }

    #[test]
    fn remaining_pct_uses_tighter_constraint() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 100_000, 4);
        // Use 3 of 4 turns → 25% remaining by turns
        budget.tick_turn();
        budget.tick_turn();
        budget.tick_turn();
        // But only ~0% tokens used → 100% remaining by tokens
        // Should return 25% (the tighter one)
        assert!((budget.remaining_pct() - 0.25).abs() < 0.01);
    }
}
