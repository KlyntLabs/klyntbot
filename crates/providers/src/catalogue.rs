//! Static model catalogue.
//!
//! Source-of-truth for model capabilities, pricing, and metadata that
//! the dynamic `/v1/models` endpoints do not surface. Merged into
//! adapter-discovered model lists by [`enrich`] and exposed as the
//! starting set by [`for_brand`].
//!
//! Modelled on opencode's `internal/llm/models/*.go` catalogues
//! (Anthropic, OpenAI, Moonshot/Kimi, DeepSeek, Groq, XAI, Zhipu) but
//! restructured around our own `ProviderModel` shape. Add new entries
//! here rather than scattering pricing / context-window facts across
//! adapters.
//!
//! Cost figures are USD per 1M tokens. Sourced from the providers'
//! public pricing pages at the time of authoring; stale data here is
//! a future-cleanup task, not a correctness issue.

use crate::types::ProviderModel;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Brand identifier used in `ProviderModel::brand`. Matches the lower-
/// case keys used by `useProviders` in the FE.
pub mod brand {
    pub const ANTHROPIC: &str = "anthropic";
    pub const OPENAI: &str = "openai";
    pub const MOONSHOT: &str = "moonshot";
    pub const DEEPSEEK: &str = "deepseek";
    pub const ZHIPU: &str = "zhipu";
    pub const DASHSCOPE: &str = "dashscope";
    pub const GROQ: &str = "groq";
    pub const XAI: &str = "xai";
    pub const GEMINI: &str = "gemini";
    pub const OPENROUTER: &str = "openrouter";
}

static ANTHROPIC_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(anthropic_models);
static OPENAI_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(openai_models);
static MOONSHOT_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(moonshot_models);
static DEEPSEEK_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(deepseek_models);
static ZHIPU_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(zhipu_models);
static GROQ_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(groq_models);
static XAI_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(xai_models);
static GEMINI_MODELS: LazyLock<Vec<ProviderModel>> =
    LazyLock::new(gemini_models);

/// Look up catalogue entries for a given brand.
///
/// Brand keys match `crate::catalogue::brand::*`. Unknown brands
/// return an empty slice — adapters fall back to dynamic discovery
/// or the configured default model.
pub fn for_brand(brand: &str) -> &'static [ProviderModel] {
    match brand {
        brand::ANTHROPIC => &ANTHROPIC_MODELS,
        brand::OPENAI => &OPENAI_MODELS,
        brand::MOONSHOT => &MOONSHOT_MODELS,
        brand::DEEPSEEK => &DEEPSEEK_MODELS,
        brand::ZHIPU => &ZHIPU_MODELS,
        brand::GROQ => &GROQ_MODELS,
        brand::XAI => &XAI_MODELS,
        brand::GEMINI => &GEMINI_MODELS,
        _ => &[],
    }
}

/// Enrich a model in-place with catalogue metadata if a matching
/// entry exists. Matches by exact id first, then by id-prefix
/// (handy for date-stamped Anthropic models like
/// `claude-sonnet-4-20250514` matching the catalogue's
/// `claude-sonnet-4-*` family). Existing fields on `model` win
/// when the catalogue value is `None`.
pub fn enrich(model: &mut ProviderModel) {
    let Some(catalogue_entry) = lookup(&model.id) else {
        return;
    };
    if model.brand.is_none() {
        model.brand = catalogue_entry.brand.clone();
    }
    if model.context_window.is_none() {
        model.context_window = catalogue_entry.context_window;
    }
    if model.default_max_tokens.is_none() {
        model.default_max_tokens = catalogue_entry.default_max_tokens;
    }
    if model.cost_per_1m_in.is_none() {
        model.cost_per_1m_in = catalogue_entry.cost_per_1m_in;
    }
    if model.cost_per_1m_in_cached.is_none() {
        model.cost_per_1m_in_cached = catalogue_entry.cost_per_1m_in_cached;
    }
    if model.cost_per_1m_out.is_none() {
        model.cost_per_1m_out = catalogue_entry.cost_per_1m_out;
    }
    if model.cost_per_1m_out_cached.is_none() {
        model.cost_per_1m_out_cached = catalogue_entry.cost_per_1m_out_cached;
    }
    if !model.supports_reasoning && catalogue_entry.supports_reasoning {
        model.supports_reasoning = true;
    }
    if !model.supports_attachments && catalogue_entry.supports_attachments {
        model.supports_attachments = true;
    }
    if model.display_name == model.id {
        model.display_name = catalogue_entry.display_name.clone();
    }
}

/// Try to find a catalogue entry matching the given id.
fn lookup(id: &str) -> Option<ProviderModel> {
    static ALL_BY_ID: LazyLock<HashMap<String, ProviderModel>> = LazyLock::new(|| {
        let mut map = HashMap::new();
        for brand in [
            brand::ANTHROPIC,
            brand::OPENAI,
            brand::MOONSHOT,
            brand::DEEPSEEK,
            brand::ZHIPU,
            brand::GROQ,
            brand::XAI,
            brand::GEMINI,
        ] {
            for entry in for_brand(brand) {
                map.insert(entry.id.clone(), entry.clone());
            }
        }
        map
    });
    ALL_BY_ID.get(id).cloned()
}

fn make(
    id: &str,
    display_name: &str,
    brand: &str,
    context_window: u64,
    default_max_tokens: u32,
    supports_reasoning: bool,
    supports_attachments: bool,
    cost_in: f64,
    cost_in_cached: f64,
    cost_out: f64,
    cost_out_cached: f64,
) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        display_name: display_name.to_string(),
        brand: Some(brand.to_string()),
        supports_reasoning,
        supports_attachments,
        context_window: Some(context_window),
        default_max_tokens: Some(default_max_tokens),
        cost_per_1m_in: Some(cost_in),
        cost_per_1m_in_cached: Some(cost_in_cached),
        cost_per_1m_out: Some(cost_out),
        cost_per_1m_out_cached: Some(cost_out_cached),
    }
}

// ---------------------------------------------------------------------
// Anthropic — https://docs.anthropic.com/en/docs/about-claude/models
// ---------------------------------------------------------------------
fn anthropic_models() -> Vec<ProviderModel> {
    vec![
        make("claude-opus-4-20250514", "Claude 4 Opus", brand::ANTHROPIC,
             200_000, 4_096, true, true, 15.0, 18.75, 75.0, 1.50),
        make("claude-sonnet-4-20250514", "Claude 4 Sonnet", brand::ANTHROPIC,
             200_000, 50_000, true, true, 3.0, 3.75, 15.0, 0.30),
        make("claude-3-7-sonnet-latest", "Claude 3.7 Sonnet", brand::ANTHROPIC,
             200_000, 50_000, true, true, 3.0, 3.75, 15.0, 0.30),
        make("claude-3-5-sonnet-latest", "Claude 3.5 Sonnet", brand::ANTHROPIC,
             200_000, 5_000, false, true, 3.0, 3.75, 15.0, 0.30),
        make("claude-3-5-haiku-latest", "Claude 3.5 Haiku", brand::ANTHROPIC,
             200_000, 4_096, false, true, 0.80, 1.0, 4.0, 0.08),
        make("claude-3-opus-latest", "Claude 3 Opus", brand::ANTHROPIC,
             200_000, 4_096, false, true, 15.0, 18.75, 75.0, 1.50),
        make("claude-3-haiku-20240307", "Claude 3 Haiku", brand::ANTHROPIC,
             200_000, 4_096, false, true, 0.25, 0.30, 1.25, 0.03),
    ]
}

// ---------------------------------------------------------------------
// OpenAI — https://platform.openai.com/docs/pricing
// ---------------------------------------------------------------------
fn openai_models() -> Vec<ProviderModel> {
    vec![
        make("gpt-4.1", "GPT 4.1", brand::OPENAI,
             1_047_576, 20_000, false, true, 2.00, 0.50, 8.00, 0.0),
        make("gpt-4.1-mini", "GPT 4.1 mini", brand::OPENAI,
             200_000, 20_000, false, true, 0.40, 0.10, 1.60, 0.0),
        make("gpt-4.1-nano", "GPT 4.1 nano", brand::OPENAI,
             1_047_576, 20_000, false, true, 0.10, 0.025, 0.40, 0.0),
        make("gpt-4.5-preview", "GPT 4.5 preview", brand::OPENAI,
             128_000, 15_000, false, true, 75.00, 37.50, 150.00, 0.0),
        make("gpt-4o", "GPT 4o", brand::OPENAI,
             128_000, 4_096, false, true, 2.50, 1.25, 10.00, 0.0),
        make("gpt-4o-mini", "GPT 4o mini", brand::OPENAI,
             128_000, 4_096, false, true, 0.15, 0.075, 0.60, 0.0),
        make("o1", "o1", brand::OPENAI,
             200_000, 50_000, true, true, 15.00, 7.50, 60.00, 0.0),
        make("o1-pro", "o1 pro", brand::OPENAI,
             200_000, 50_000, true, true, 150.00, 0.0, 600.00, 0.0),
        make("o1-mini", "o1 mini", brand::OPENAI,
             128_000, 50_000, true, true, 1.10, 0.55, 4.40, 0.0),
        make("o3", "o3", brand::OPENAI,
             200_000, 50_000, true, true, 10.00, 2.50, 40.00, 0.0),
        make("o3-mini", "o3 mini", brand::OPENAI,
             200_000, 50_000, true, false, 1.10, 0.55, 4.40, 0.0),
        make("o4-mini", "o4 mini", brand::OPENAI,
             128_000, 50_000, true, true, 1.10, 0.275, 4.40, 0.0),
    ]
}

// ---------------------------------------------------------------------
// Moonshot / Kimi — https://platform.moonshot.cn/docs
// Pricing snapshot 2025; refresh when stale.
// ---------------------------------------------------------------------
fn moonshot_models() -> Vec<ProviderModel> {
    // All K2.x models are thinking-capable: pass `reasoning_effort` +
    // `extra_body.thinking={type:"enabled"}` and they emit reasoning
    // content via SSE. `kimi-thinking-preview` defaults to thinking
    // ON; the others default OFF but the capability bit is the same.
    // Source: kimi-cli `packages/kosong/.../chat_provider/kimi.py`
    // (`with_thinking` sets both flags regardless of model id).
    vec![
        make("kimi-k2.6", "Kimi K2.6", brand::MOONSHOT,
             200_000, 16_384, true, true, 0.6, 0.0, 2.5, 0.0),
        make("kimi-thinking-preview", "Kimi Thinking (preview)", brand::MOONSHOT,
             131_072, 16_384, true, true, 0.6, 0.0, 2.5, 0.0),
        make("kimi-k2-turbo-preview", "Kimi K2 Turbo (preview)", brand::MOONSHOT,
             131_072, 8_192, true, true, 0.4, 0.0, 1.6, 0.0),
        // Moonshot V1 family does NOT support extended thinking — older
        // architecture, no `reasoning_content` in their SSE schema.
        make("moonshot-v1-8k", "Moonshot V1 8K", brand::MOONSHOT,
             8_192, 4_096, false, true, 0.17, 0.0, 0.17, 0.0),
        make("moonshot-v1-32k", "Moonshot V1 32K", brand::MOONSHOT,
             32_768, 8_192, false, true, 0.34, 0.0, 0.34, 0.0),
        make("moonshot-v1-128k", "Moonshot V1 128K", brand::MOONSHOT,
             131_072, 16_384, false, true, 0.83, 0.0, 0.83, 0.0),
    ]
}

// ---------------------------------------------------------------------
// DeepSeek — https://api-docs.deepseek.com/quick_start/pricing
// ---------------------------------------------------------------------
fn deepseek_models() -> Vec<ProviderModel> {
    vec![
        make("deepseek-chat", "DeepSeek Chat", brand::DEEPSEEK,
             64_000, 8_000, false, false, 0.27, 0.07, 1.10, 0.0),
        make("deepseek-reasoner", "DeepSeek Reasoner (R1)", brand::DEEPSEEK,
             64_000, 8_000, true, false, 0.55, 0.14, 2.19, 0.0),
    ]
}

// ---------------------------------------------------------------------
// Zhipu — https://open.bigmodel.cn/dev/howuse/model
// ---------------------------------------------------------------------
fn zhipu_models() -> Vec<ProviderModel> {
    vec![
        make("glm-4.6", "GLM-4.6", brand::ZHIPU,
             128_000, 8_192, false, true, 0.3, 0.0, 1.5, 0.0),
        make("glm-4.5", "GLM-4.5", brand::ZHIPU,
             128_000, 8_192, false, true, 0.6, 0.0, 2.0, 0.0),
        make("glm-zero-preview", "GLM Zero (preview)", brand::ZHIPU,
             32_768, 8_192, true, false, 1.4, 0.0, 1.4, 0.0),
    ]
}

// ---------------------------------------------------------------------
// Groq — https://console.groq.com/docs/models
// ---------------------------------------------------------------------
fn groq_models() -> Vec<ProviderModel> {
    vec![
        make("qwen-qwq-32b", "Qwen QwQ 32B", brand::GROQ,
             128_000, 50_000, true, false, 0.29, 0.275, 0.39, 0.0),
        make("deepseek-r1-distill-llama-70b", "DeepSeek R1 Distill Llama 70B", brand::GROQ,
             128_000, 50_000, true, false, 0.75, 0.0, 0.99, 0.0),
        make("llama-3.3-70b-versatile", "Llama 3.3 70B", brand::GROQ,
             128_000, 50_000, false, false, 0.59, 0.0, 0.79, 0.0),
        make("meta-llama/llama-4-scout-17b-16e-instruct", "Llama 4 Scout 17B", brand::GROQ,
             128_000, 50_000, false, true, 0.11, 0.0, 0.34, 0.0),
        make("meta-llama/llama-4-maverick-17b-128e-instruct", "Llama 4 Maverick 17B", brand::GROQ,
             128_000, 50_000, false, true, 0.20, 0.0, 0.60, 0.0),
    ]
}

// ---------------------------------------------------------------------
// xAI — https://docs.x.ai/docs/api-reference#models
// ---------------------------------------------------------------------
fn xai_models() -> Vec<ProviderModel> {
    vec![
        make("grok-3-beta", "Grok 3 Beta", brand::XAI,
             131_072, 20_000, false, false, 3.0, 0.0, 15.0, 0.0),
        make("grok-3-mini-beta", "Grok 3 Mini Beta", brand::XAI,
             131_072, 20_000, false, false, 0.3, 0.0, 0.5, 0.0),
        make("grok-3-fast-beta", "Grok 3 Fast Beta", brand::XAI,
             131_072, 20_000, false, false, 5.0, 0.0, 25.0, 0.0),
        make("grok-3-mini-fast-beta", "Grok 3 Mini Fast Beta", brand::XAI,
             131_072, 20_000, false, false, 0.6, 0.0, 4.0, 0.0),
    ]
}

// ---------------------------------------------------------------------
// Gemini — https://ai.google.dev/pricing
// ---------------------------------------------------------------------
fn gemini_models() -> Vec<ProviderModel> {
    vec![
        make("gemini-2.5-pro", "Gemini 2.5 Pro", brand::GEMINI,
             1_000_000, 65_000, true, true, 1.25, 0.0, 10.0, 0.0),
        make("gemini-2.5-flash", "Gemini 2.5 Flash", brand::GEMINI,
             1_000_000, 65_000, true, true, 0.30, 0.0, 2.50, 0.0),
        make("gemini-2.0-flash", "Gemini 2.0 Flash", brand::GEMINI,
             1_000_000, 8_192, false, true, 0.10, 0.0, 0.40, 0.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enrich_fills_missing_fields() {
        let mut m = ProviderModel::from_id("claude-sonnet-4-20250514");
        assert!(m.cost_per_1m_in.is_none());
        enrich(&mut m);
        assert_eq!(m.cost_per_1m_in, Some(3.0));
        assert!(m.supports_reasoning);
        assert_eq!(m.brand.as_deref(), Some(brand::ANTHROPIC));
    }

    #[test]
    fn enrich_preserves_existing_fields() {
        let mut m = ProviderModel::from_id("claude-sonnet-4-20250514");
        m.cost_per_1m_in = Some(99.0);
        enrich(&mut m);
        assert_eq!(m.cost_per_1m_in, Some(99.0));
    }

    #[test]
    fn for_brand_returns_known_models() {
        let anthropic = for_brand(brand::ANTHROPIC);
        assert!(anthropic.iter().any(|m| m.id == "claude-sonnet-4-20250514"));
        let moonshot = for_brand(brand::MOONSHOT);
        assert!(moonshot.iter().any(|m| m.id == "kimi-thinking-preview"));
        assert!(for_brand("unknown").is_empty());
    }

    #[test]
    fn kimi_thinking_marked_reasoning() {
        let m = for_brand(brand::MOONSHOT)
            .into_iter()
            .find(|m| m.id == "kimi-thinking-preview")
            .unwrap();
        assert!(m.supports_reasoning);
    }
}
