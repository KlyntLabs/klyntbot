//! Centralized model pricing. Single source of truth for cost tracking,
//! budget enforcement, and cost-aware autotuning experiments.
//!
//! ## Adding a new model
//!
//! 1. Append a `ModelPricing` entry to [`MODEL_PRICING`].
//! 2. Order matters — list more-specific patterns first (e.g. `"opus-4-7-1m"`
//!    before `"opus-4"`); lookup returns the first substring match.
//! 3. For 1M-context premium tiers, set `long_context` with the threshold and
//!    multipliers from the provider's pricing page.
//! 4. Run `cargo nextest run -p common` — the table integrity tests
//!    catch ordering mistakes and missing common families.
//!
//! Prices are in USD per 1,000,000 tokens, captured from each provider's
//! public pricing page. Update when providers publish a change.
//!
//! Last verified: 2026-04 — Anthropic, OpenAI, DeepSeek, Moonshot.
//!
//! ## API
//!
//! - [`lookup`] — find the pricing entry that matches a model id.
//! - [`cost_for`] — compute `(input_cost_usd, output_cost_usd)` for a model
//!   and token counts; returns `None` for unrecognized models so callers can
//!   distinguish "free / unknown" from "$0".

/// Optional premium pricing tier triggered when input tokens exceed a threshold.
/// Used for Anthropic's 1M-context Opus/Sonnet variants.
#[derive(Debug, Clone, Copy)]
pub struct LongContextTier {
    /// Tokens above which the multipliers apply.
    pub input_threshold_tokens: u64,
    /// Multiplier applied to the base input rate when above threshold.
    pub input_multiplier: f64,
    /// Multiplier applied to the base output rate when above threshold.
    pub output_multiplier: f64,
}

/// One pricing entry — substring-matched against the model id.
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    /// Substring matched (case-insensitive) against the model id.
    pub model_pattern: &'static str,
    /// USD per 1M input tokens (base tier).
    pub input_per_mtok: f64,
    /// USD per 1M output tokens (base tier).
    pub output_per_mtok: f64,
    /// USD per 1M cache-read tokens.
    pub cache_read_per_mtok: f64,
    /// USD per 1M cache-write tokens.
    pub cache_write_per_mtok: f64,
    /// Long-context premium tier, if the model supports one.
    pub long_context: Option<LongContextTier>,
    /// Human-readable label for display.
    pub display_name: &'static str,
}

const ANTHROPIC_1M_TIER: LongContextTier = LongContextTier {
    input_threshold_tokens: 200_000,
    input_multiplier: 2.0,
    output_multiplier: 1.5,
};

/// Pricing table — order matters (most-specific first).
///
/// Reference (last verified 2026-04):
/// - <https://www.anthropic.com/pricing>
/// - <https://openai.com/api/pricing>
/// - <https://api-docs.deepseek.com/quick_start/pricing>
/// - <https://platform.moonshot.ai/pricing>
pub static MODEL_PRICING: &[ModelPricing] = &[
    // === Anthropic — Opus family ===
    ModelPricing {
        model_pattern: "opus-4-7[1m]",
        input_per_mtok: 15.0,
        output_per_mtok: 75.0,
        cache_read_per_mtok: 1.50,
        cache_write_per_mtok: 18.75,
        long_context: Some(ANTHROPIC_1M_TIER),
        display_name: "Claude Opus 4.7 (1M context)",
    },
    ModelPricing {
        model_pattern: "opus-4-7",
        input_per_mtok: 15.0,
        output_per_mtok: 75.0,
        cache_read_per_mtok: 1.50,
        cache_write_per_mtok: 18.75,
        long_context: None,
        display_name: "Claude Opus 4.7",
    },
    ModelPricing {
        model_pattern: "opus-4-6",
        input_per_mtok: 15.0,
        output_per_mtok: 75.0,
        cache_read_per_mtok: 1.50,
        cache_write_per_mtok: 18.75,
        long_context: None,
        display_name: "Claude Opus 4.6",
    },
    ModelPricing {
        model_pattern: "opus-4",
        input_per_mtok: 15.0,
        output_per_mtok: 75.0,
        cache_read_per_mtok: 1.50,
        cache_write_per_mtok: 18.75,
        long_context: None,
        display_name: "Claude Opus 4.x",
    },
    ModelPricing {
        model_pattern: "opus-3",
        input_per_mtok: 15.0,
        output_per_mtok: 75.0,
        cache_read_per_mtok: 1.50,
        cache_write_per_mtok: 18.75,
        long_context: None,
        display_name: "Claude Opus 3",
    },
    // === Anthropic — Sonnet family ===
    ModelPricing {
        model_pattern: "sonnet-4-7",
        input_per_mtok: 3.0,
        output_per_mtok: 15.0,
        cache_read_per_mtok: 0.30,
        cache_write_per_mtok: 3.75,
        long_context: Some(ANTHROPIC_1M_TIER),
        display_name: "Claude Sonnet 4.7",
    },
    ModelPricing {
        model_pattern: "sonnet-4-6",
        input_per_mtok: 3.0,
        output_per_mtok: 15.0,
        cache_read_per_mtok: 0.30,
        cache_write_per_mtok: 3.75,
        long_context: Some(ANTHROPIC_1M_TIER),
        display_name: "Claude Sonnet 4.6",
    },
    ModelPricing {
        model_pattern: "sonnet",
        input_per_mtok: 3.0,
        output_per_mtok: 15.0,
        cache_read_per_mtok: 0.30,
        cache_write_per_mtok: 3.75,
        long_context: None,
        display_name: "Claude Sonnet",
    },
    // === Anthropic — Haiku family ===
    ModelPricing {
        model_pattern: "haiku-4-5",
        input_per_mtok: 1.0,
        output_per_mtok: 5.0,
        cache_read_per_mtok: 0.08,
        cache_write_per_mtok: 1.0,
        long_context: None,
        display_name: "Claude Haiku 4.5",
    },
    ModelPricing {
        model_pattern: "haiku-4",
        input_per_mtok: 1.0,
        output_per_mtok: 5.0,
        cache_read_per_mtok: 0.08,
        cache_write_per_mtok: 1.0,
        long_context: None,
        display_name: "Claude Haiku 4",
    },
    // Legacy Claude 3 Haiku — specific match before the generic "haiku" fallback.
    ModelPricing {
        model_pattern: "claude-3-haiku-20240307",
        input_per_mtok: 0.25,
        output_per_mtok: 1.25,
        cache_read_per_mtok: 0.03,
        cache_write_per_mtok: 0.30,
        long_context: None,
        display_name: "Claude 3 Haiku (legacy)",
    },
    ModelPricing {
        model_pattern: "haiku",
        input_per_mtok: 0.80,
        output_per_mtok: 4.0,
        cache_read_per_mtok: 0.08,
        cache_write_per_mtok: 1.0,
        long_context: None,
        display_name: "Claude Haiku 3.x",
    },
    // === OpenAI ===
    ModelPricing {
        model_pattern: "gpt-4o-mini",
        input_per_mtok: 0.15,
        output_per_mtok: 0.60,
        cache_read_per_mtok: 0.075,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "GPT-4o mini",
    },
    ModelPricing {
        model_pattern: "gpt-4-mini",
        input_per_mtok: 0.15,
        output_per_mtok: 0.60,
        cache_read_per_mtok: 0.075,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "GPT-4 mini",
    },
    ModelPricing {
        model_pattern: "gpt-4o",
        input_per_mtok: 2.50,
        output_per_mtok: 10.0,
        cache_read_per_mtok: 1.25,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "GPT-4o",
    },
    ModelPricing {
        model_pattern: "gpt-4",
        input_per_mtok: 2.50,
        output_per_mtok: 10.0,
        cache_read_per_mtok: 1.25,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "GPT-4",
    },
    // === DeepSeek ===
    ModelPricing {
        model_pattern: "deepseek-coder",
        input_per_mtok: 0.27,
        output_per_mtok: 1.10,
        cache_read_per_mtok: 0.07,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "DeepSeek Coder",
    },
    ModelPricing {
        model_pattern: "deepseek-reasoner",
        input_per_mtok: 0.55,
        output_per_mtok: 2.19,
        cache_read_per_mtok: 0.14,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "DeepSeek Reasoner",
    },
    ModelPricing {
        model_pattern: "deepseek",
        input_per_mtok: 0.27,
        output_per_mtok: 1.10,
        cache_read_per_mtok: 0.07,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "DeepSeek Chat",
    },
    // === Google Gemini ===
    ModelPricing {
        model_pattern: "gemini-2.0-flash",
        input_per_mtok: 0.10,
        output_per_mtok: 0.40,
        cache_read_per_mtok: 0.025,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "Gemini 2.0 Flash",
    },
    ModelPricing {
        model_pattern: "gemini-1.5-pro",
        input_per_mtok: 1.25,
        output_per_mtok: 5.0,
        cache_read_per_mtok: 0.315,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "Gemini 1.5 Pro",
    },
    ModelPricing {
        model_pattern: "gemini-1.5-flash",
        input_per_mtok: 0.075,
        output_per_mtok: 0.30,
        cache_read_per_mtok: 0.01875,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "Gemini 1.5 Flash",
    },
    // === Mistral ===
    ModelPricing {
        model_pattern: "mistral-large",
        input_per_mtok: 2.0,
        output_per_mtok: 6.0,
        cache_read_per_mtok: 0.0,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "Mistral Large",
    },
    ModelPricing {
        model_pattern: "mistral-small",
        input_per_mtok: 0.10,
        output_per_mtok: 0.30,
        cache_read_per_mtok: 0.0,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "Mistral Small",
    },
    // === Moonshot Kimi ===
    ModelPricing {
        model_pattern: "kimi",
        input_per_mtok: 0.15,
        output_per_mtok: 2.50,
        cache_read_per_mtok: 0.0,
        cache_write_per_mtok: 0.0,
        long_context: None,
        display_name: "Kimi",
    },
];

/// Find the first pricing entry whose `model_pattern` is a substring of `model`
/// (case-insensitive). Returns `None` for unrecognized models — callers should
/// treat that as "skip cost calculation" rather than zero.
#[must_use]
pub fn lookup(model: &str) -> Option<&'static ModelPricing> {
    let m = model.to_ascii_lowercase();
    MODEL_PRICING.iter().find(|p| m.contains(p.model_pattern))
}

/// Compute `(input_cost_usd, output_cost_usd)` for a model and token counts.
/// Applies the long-context tier when input tokens exceed its threshold.
/// Returns `None` for unrecognized models.
#[must_use]
pub fn cost_for(model: &str, input_tokens: u64, output_tokens: u64) -> Option<(f64, f64)> {
    let p = lookup(model)?;
    let (in_rate, out_rate) = match p.long_context {
        Some(lc) if input_tokens > lc.input_threshold_tokens => (
            p.input_per_mtok * lc.input_multiplier,
            p.output_per_mtok * lc.output_multiplier,
        ),
        _ => (p.input_per_mtok, p.output_per_mtok),
    };
    Some((
        input_tokens as f64 * in_rate / 1_000_000.0,
        output_tokens as f64 * out_rate / 1_000_000.0,
    ))
}

/// Compute total cost (input + output + cache read + cache write) for a model.
/// Applies the long-context tier when input tokens exceed its threshold.
/// Returns `None` for unrecognized models.
#[must_use]
pub fn cost_with_cache_for(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
) -> Option<f64> {
    let p = lookup(model)?;
    let (in_rate, out_rate) = match p.long_context {
        Some(lc) if input_tokens > lc.input_threshold_tokens => (
            p.input_per_mtok * lc.input_multiplier,
            p.output_per_mtok * lc.output_multiplier,
        ),
        _ => (p.input_per_mtok, p.output_per_mtok),
    };
    Some(
        input_tokens as f64 * in_rate / 1_000_000.0
            + output_tokens as f64 * out_rate / 1_000_000.0
            + cache_read_tokens as f64 * p.cache_read_per_mtok / 1_000_000.0
            + cache_write_tokens as f64 * p.cache_write_per_mtok / 1_000_000.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_47_matches_before_opus_4() {
        let p = lookup("claude-opus-4-7").unwrap();
        assert_eq!(p.display_name, "Claude Opus 4.7");
        assert_eq!(p.input_per_mtok, 15.0);
    }

    #[test]
    fn opus_46_matches_specific_entry() {
        let p = lookup("claude-opus-4-6").unwrap();
        assert_eq!(p.display_name, "Claude Opus 4.6");
    }

    #[test]
    fn opus_47_1m_long_context_kicks_in_above_200k() {
        // 250k input, 50k output on the 1M variant.
        let (i, o) = cost_for("claude-opus-4-7[1m]", 250_000, 50_000).unwrap();
        // input: 250k * $30/1M = $7.50 (2× multiplier)
        // output: 50k * $112.50/1M = $5.625 (1.5× multiplier)
        assert!((i - 7.50).abs() < 1e-6);
        assert!((o - 5.625).abs() < 1e-6);
    }

    #[test]
    fn opus_47_1m_below_threshold_uses_base_rate() {
        let (i, o) = cost_for("claude-opus-4-7[1m]", 100_000, 50_000).unwrap();
        assert!((i - 1.50).abs() < 1e-6); // 100k × $15/1M
        assert!((o - 3.75).abs() < 1e-6); // 50k × $75/1M
    }

    #[test]
    fn sonnet_46_matches() {
        let p = lookup("claude-sonnet-4-6").unwrap();
        assert_eq!(p.input_per_mtok, 3.0);
        assert_eq!(p.output_per_mtok, 15.0);
    }

    #[test]
    fn deepseek_coder_matches_specific_before_chat() {
        let p = lookup("deepseek-coder").unwrap();
        assert_eq!(p.display_name, "DeepSeek Coder");
    }

    #[test]
    fn unknown_returns_none() {
        assert!(lookup("some-model-that-does-not-exist").is_none());
        assert!(cost_for("nope", 1000, 500).is_none());
    }

    #[test]
    fn case_insensitive_lookup() {
        assert!(lookup("CLAUDE-OPUS-4-7").is_some());
        assert!(lookup("Claude-Opus-4-7").is_some());
    }

    #[test]
    fn pattern_ordering_invariant_specific_before_generic() {
        // For each generic pattern, every more-specific pattern that matches it
        // must appear earlier in the table. Otherwise lookup returns the
        // generic pricing for what should be a specific tier.
        for (i, generic) in MODEL_PRICING.iter().enumerate() {
            for (j, specific) in MODEL_PRICING.iter().enumerate() {
                if i == j {
                    continue;
                }
                if specific.model_pattern.contains(generic.model_pattern)
                    && specific.model_pattern != generic.model_pattern
                {
                    assert!(
                        j < i,
                        "ordering violation: '{}' is more specific than '{}' \
                         but appears later in MODEL_PRICING",
                        specific.model_pattern,
                        generic.model_pattern
                    );
                }
            }
        }
    }

    #[test]
    fn cache_costs_included_in_total() {
        let cost = cost_with_cache_for("claude-sonnet-4", 1_000_000, 100_000, 50_000, 20_000)
            .unwrap();
        // input: 1M * $3  = $3.00
        // output: 0.1M * $15 = $1.50
        // cache read: 0.05M * $0.30 = $0.015
        // cache write: 0.02M * $3.75 = $0.075
        // total = $4.59
        assert!((cost - 4.59).abs() < 1e-6, "expected $4.59, got ${cost}");
    }

    #[test]
    fn legacy_claude_3_haiku_has_lower_price() {
        let p = lookup("claude-3-haiku-20240307").unwrap();
        assert_eq!(p.display_name, "Claude 3 Haiku (legacy)");
        assert!((p.input_per_mtok - 0.25).abs() < 1e-6);
        assert!((p.output_per_mtok - 1.25).abs() < 1e-6);
    }

    #[test]
    fn gemini_models_present() {
        assert!(lookup("gemini-2.0-flash").is_some());
        assert!(lookup("gemini-1.5-pro").is_some());
        assert!(lookup("gemini-1.5-flash").is_some());
    }

    #[test]
    fn mistral_models_present() {
        assert!(lookup("mistral-large-latest").is_some());
        assert!(lookup("mistral-small-latest").is_some());
    }

    #[test]
    fn deepseek_reasoner_matches_before_generic_deepseek() {
        let p = lookup("deepseek-reasoner").unwrap();
        assert_eq!(p.display_name, "DeepSeek Reasoner");
        assert!((p.input_per_mtok - 0.55).abs() < 1e-6);
    }

    #[test]
    fn unknown_model_returns_none_for_cache_cost() {
        assert!(cost_with_cache_for("totally-unknown", 1000, 500, 100, 50).is_none());
    }
}
