# Simulator Real LLM Provider Integration

## Goal

Replace the mock `SimulationProvider` with a real LLM (DeepSeek for initial development, configurable for any provider) so the simulator produces genuine evaluation metrics. The mock provider stays as the default for CI/unit tests; real LLM mode is opt-in via config + env var.

## Architecture

The `AgentHarness` gains a provider factory that reads `agent_provider` and `agent_model` from `SimulationConfig`. When set to a real provider, it constructs an `OpenAiCompatProvider` (for DeepSeek, OpenAI, Groq, etc.) or `AnthropicNativeProvider` (for Claude) from the corresponding env var API key. The `IntentAnalyzer` drops shadow mode when a real LLM is used, enabling genuine intent classification. An `AdversarialProviderWrapper` preserves the error injection layer by wrapping whatever inner provider is selected.

## Provider Configuration

### SimulationConfig additions

```toml
[simulation]
agent_mode = true
agent_provider = "deepseek"                    # "mock" (default) | "deepseek" | "anthropic" | "openai" | ...
agent_model = "deepseek-chat"                  # model name for the selected provider
```

When `agent_provider = "mock"` or omitted, the existing `SimulationProvider` is used (free, deterministic, no API key needed). This is the CI default.

### API Key Resolution

API keys are resolved from environment variables, never from TOML:

| Provider | Env Var | API Base |
|----------|---------|----------|
| `deepseek` | `DEEPSEEK_API_KEY` | `https://api.deepseek.com` |
| `anthropic` | `ANTHROPIC_API_KEY` | `https://api.anthropic.com/v1` |
| `openai` | `OPENAI_API_KEY` | `https://api.openai.com/v1` |
| `groq` | `GROQ_API_KEY` | `https://api.groq.com/openai/v1` |

When a real provider is configured but the env var is missing, the harness falls back to mock with a warning.

## Provider Factory

A new `create_provider()` function in `agent_harness.rs`:

```rust
fn create_provider(
    provider_name: &str,
    model: &str,
    seed: u64,
) -> common::Result<DynProvider>
```

- `"mock"` -> `SimulationProvider::new(seed)`
- `"deepseek"` -> `OpenAiCompatProvider::new("https://api.deepseek.com", env_key, model)`
- `"anthropic"` -> `AnthropicNativeProvider::new(env_key, base_url, model)`
- `"openai"` -> `OpenAiCompatProvider::new("https://api.openai.com/v1", env_key, model)`
- Others -> `OpenAiCompatProvider` with provider-specific base URL from registry

Returns `Err` if the env var is missing (caller handles fallback to mock).

## AdversarialProviderWrapper

Wraps any `DynProvider` to inject malformed responses at a configurable rate. Replaces the `provider_error_rate` field that was on `SimulationProvider`:

```rust
struct AdversarialProviderWrapper {
    inner: DynProvider,
    error_rate: f64,
    rng: Mutex<StdRng>,
}
```

Implements `LlmProvider` by delegating `chat()` / `chat_stream()` to `inner`, but with probability `error_rate` returns a malformed response instead (same 4 types: typo tool name, invalid JSON args, empty ID, nonexistent tool).

When `error_rate = 0.0`, the wrapper is a no-op pass-through.

## IntentAnalyzer Mode

When `agent_provider != "mock"`:
- Remove `with_shadow_mode()` from the IntentAnalyzer construction
- The classifier makes real LLM calls for intent analysis
- This means routing accuracy reflects genuine AI reasoning, not heuristic keyword matching

When `agent_provider == "mock"`:
- Keep `with_shadow_mode()` (existing behavior)
- Heuristic-only classification

## Test Scenarios

### CI (mock, free)
All existing tests use `agent_provider = "mock"` by default. No env var needed. Fast, deterministic, runs in CI.

### Development (real LLM, cheap)
- 7-day smoke test: ~40 messages, ~$0.001 with DeepSeek
- New `software_engineer_1mo.toml`: 30-day scenario, ~150 messages, ~$0.01 with DeepSeek

### Evaluation (real LLM, full)
- 12-month scenario with `agent_provider = "deepseek"`: ~1700 messages, ~$0.10-0.50

### Gating
Real LLM tests are gated on the env var. If `DEEPSEEK_API_KEY` is not set, the test either skips or falls back to mock. This prevents CI failures and accidental cost.

## File Changes

### Modified files
- `crates/simulator/src/scenario.rs` — add `agent_provider`, `agent_model` fields to `SimulationConfig`
- `crates/simulator/src/agent_harness.rs` — `create_provider()` factory, remove shadow mode for real LLM, accept provider config
- `crates/simulator/src/harness.rs` — pass provider config to AgentHarness construction, pass `provider_error_rate` to wrapper
- `crates/simulator/src/providers/simulation_provider.rs` — remove `provider_error_rate` field (moved to wrapper)

### New files
- `crates/simulator/src/providers/adversarial_wrapper.rs` — AdversarialProviderWrapper

### Scenario files
- `tests/simulation/scenarios/software_engineer_12mo.toml` — add `agent_provider = "deepseek"`, `agent_model = "deepseek-chat"`
- `tests/simulation/scenarios/software_engineer_1mo.toml` — new 30-day scenario for development testing

## Verification

Development testing (with `DEEPSEEK_API_KEY` set):
- 7-day smoke test with real LLM completes without errors
- 30-day scenario produces non-trivial metrics (routing_accuracy != 0.5, tool_selection != 1.0)
- multi_turn_coherence reflects actual conversation quality
- adversarial_resilience < 1.0 (real LLM actually struggles with some adversarial messages)

CI testing (no env var):
- All existing tests pass unchanged (mock provider)
- No API calls made
