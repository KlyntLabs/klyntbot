# Phase 3 Architecture Verification — Agent 10

**Crates:** `desktop`, `mcp`, `providers`, `storage`, `tools-core`  
**Date:** 2026-05-16  
**Method:** End-to-end doc read → full `src/` tree listing → per-module source verification → signature/constant/struct comparison → TODO/FIXME inventory.

---

## Summary

| Crate | Verdict | Drift Count | Severity |
|---|---|---|---|
| `desktop` | ⚠️ Minor drift | 3 | Low |
| `mcp` | ⚠️ Moderate drift | 4 | Medium |
| `providers` | 🔴 Significant drift | 10+ | High |
| `storage` | ✅ Accurate | 1 cosmetic | Low |
| `tools-core` | ⚠️ Moderate drift | 2 | Medium |

**Most critical drifts:**
1. **`providers` `LlmProvider` trait** — doc documents an older API surface (method names, signatures, `Box<dyn>`) that does not match the current source.
2. **`providers` `ProviderCapabilities` struct** — completely different field set between doc and source.
3. **`providers` constants** — `DEFAULT_CONTEXT_WINDOW` (doc 200K vs source 128K), `CacheAnchor` variants, `ChatParams` fields.
4. **`mcp` `McpCircuitBreaker::start_health_check`** — documented on wrong struct; actually lives on `McpManager`.
5. **`tools-core` interceptor** — doc documents `after`/`run_after` methods that do not exist.

---

## `desktop`

### Module tree — verified ✅
Doc tree matches `src/` exactly: `commands/`, `commands/oauth/`, `oauth/`, `tray_countdown.rs`, `focus_timer.rs`, `lazy_window.rs`, `specta_builder.rs`, `shortcuts.rs`.

### 17-step startup sequence — ⚠️ minor drift
Doc presents:
```rust
fn main() -> Result<(), Box<dyn Error>> { ... }
fn run_desktop_app() -> Result<()> { ... }
```

**Actual source** (`crates/desktop/src/main.rs`):
```rust
fn main() { ... }           // returns ()
fn run_desktop_app() { ... } // returns ()
```

Doc omits:
- `tauri::async_runtime::set(handle)` immediately after leaking the tokio runtime.
- `Box::leak(Box::new(rt))` is shown as `Box::leak(Box::new(...))` but doc writes it as `Box::leak(Box::new(tokio::runtime::Builder::...))` — same intent.

The 17 conceptual steps broadly match the source flow; only return types are wrong.

### CI guard tests — ⚠️ count mismatch
Doc claims **4 CI guard tests**:
- `bindings_are_current`
- `no_double_registration`
- `no_raw_tauri_command_outside_macros`
- `registration_drift`

**Actual:** 5 test files in `crates/desktop/tests/`:
1. `bindings_are_current.rs`
2. `no_double_registration.rs`
3. `no_raw_tauri_command_outside_macros.rs`
4. `registration_drift.rs`
5. `specta_builder_smoke.rs` ← **not mentioned in doc**

The 5th is a lightweight smoke test (`builder_yields_a_handler`) not a CI guard. Doc is accurate about the 4 guards but should note the smoke test exists.

### Secondary windows — ✅ verified
All 5 windows (`launcher`, `tray`, `distraction-overlay`, `voice-orb`, `coding:{repo_id}`) with sizes and behaviors match source exactly.

### `specta_builder.rs` — ✅ verified
- `KLYNT_COMMANDS` linkme slice, `klynt_invoke_handler()` HashMap dispatch, ~465 commands, ~70 events all match.

---

## `mcp`

### Module tree — ✅ verified
Doc tree matches exactly: `allowlist.rs`, `client/{mod.rs,handler.rs,transport.rs,circuit_breaker.rs}`, `server/{mod.rs,approval.rs}`, plus `klyntbot-server/src/` bridge files.

### `McpCircuitBreaker` — 🔴 drift
**Doc claims:**
```rust
impl McpCircuitBreaker {
    pub fn start_health_check(self: &Arc<Self>, manager: Arc<McpManager>) -> JoinHandle<()>;
}
```

**Actual source** (`crates/mcp/src/client/circuit_breaker.rs`):
No such method exists. `McpCircuitBreaker` only has:
- `new(threshold, cooldown_secs)`
- `is_open(&self, server) -> bool`
- `record_failure(&self, server) -> bool`
- `record_success(&self, server)`
- `cooldown_expired(&self, server) -> bool`
- `cleanup(&self)`

**`start_health_check` actually lives on `McpManager`** (or as an associated function taking `Arc<RwLock<Option<McpManager>>>`) in `crates/mcp/src/client/manager.rs` line 426. It takes `manager`, `registry`, and `cancel` token — not `self`.

### `McpChannelAllowlist` — 🔴 drift
**Doc claims:**
```rust
pub fn is_server_allowed(&self, channel: &str, server: &str) -> bool;
```

**Actual source** (`crates/mcp/src/allowlist.rs`):
```rust
pub fn decide(&self, channel: &str, server: &str) -> AllowDecision;
```
where `AllowDecision` is an enum with `Allowed` / `Denied` variants.

No `is_server_allowed` method exists.

### `McpApprovalChannel` — ⚠️ return-type drift
**Doc claims:**
```rust
async fn request(&self, req: ApprovalRequest) -> Result<ApprovalDecision>;
```

**Actual source:**
```rust
async fn request(&self, req: ApprovalRequest) -> ApprovalDecision;
```

Returns the enum directly, not `Result`.

### MCP resources — ✅ verified
Doc claims 4 resources: `klyntbot://status`, `klyntbot://memory/recent`, `klyntbot://tasks/today`, `klyntbot://config/skills`.

Source `crates/klyntbot-server/src/handler.rs` `build_resources()` confirms exactly these 4.

### `AgentBridge` auto-decline — ✅ verified
Source `crates/klyntbot-server/src/bridge/agent.rs` contains `// Auto-decline: MCP has no interactive prompt capability (yet).` and a `test_auto_decline_interaction` test.

### `EXPLICIT_TOOL_ALLOWLIST` — ✅ verified
16 hardcoded tool names in `crates/config/src/schema/mcp.rs` match doc list exactly.

---

## `providers`

### Module tree — ✅ verified
4 adapters (`anthropic_native`, `openai_compat`, `transcription`, `noop`) + `types.rs`, `manager.rs`, `registry.rs`, `factory.rs` all present.

### `LlmProvider` trait — 🔴 major drift
**Doc documents:**
```rust
#[async_trait]
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn health(&self) -> ProviderHealth;
    async fn chat_completion(&self, messages: Vec<Message>, params: ChatParams) -> Result<LlmResponse, ProviderError>;
    fn chat_completion_stream(&self, messages: Vec<Message>, params: ChatParams) -> LlmStream;
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, ProviderError>;
}
pub type DynProvider = Box<dyn LlmProvider>;
```

**Actual source** (`crates/providers/src/types.rs`):
```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: &[Message], tools: Option<&[Value]>, params: &ChatParams, cache_breakpoints: &[CacheBreakpoint]) -> Result<LlmResponse>;
    async fn chat_stream(&self, messages: &[Message], tools: Option<&[Value]>, params: &ChatParams, cache_breakpoints: &[CacheBreakpoint]) -> Result<LlmStream>;
    fn default_model(&self) -> &str;
    fn name(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn supports_streaming(&self) -> bool;
    fn context_window(&self) -> usize;
    async fn count_tokens(&self, messages: &[Message], model: Option<&str>) -> Result<usize>;
    async fn health_check(&self) -> Result<ProviderHealth>;
    fn classifier_provider(&self) -> Option<&'static ProviderSpec>;
    async fn list_models(&self) -> Result<Vec<String>>;
}
pub type DynProvider = Arc<dyn LlmProvider>;
```

**Drifts:**
- Method names: `chat_completion` → `chat`, `chat_completion_stream` → `chat_stream`, `health` → `health_check`, `model` → `default_model`
- Signatures: doc omits `tools`, `cache_breakpoints` parameters; uses owned `Vec<Message>` instead of `&[Message]`; `ChatParams` by value vs reference
- Missing methods in doc: `count_tokens`, `supports_streaming`, `context_window`, `classifier_provider`, `list_models`
- `DynProvider` is `Arc<dyn>` not `Box<dyn>`
- No `embed` method exists on `LlmProvider` (it may exist on a different trait or be unimplemented)

### Adapter constructors — 🔴 drift
**AnthropicNativeProvider:**
- Doc: `new(api_key, model, base_url, beta_headers)`
- Source: `new(api_key: Secret<String>, base_url: String, model: String)` with builder methods `.with_api_version()`, `.with_cache_system_prompt()`, `.with_extended_thinking()`

**OpenAiCompatProvider:**
- Doc: `new(api_key, base_url, model, capabilities)`
- Source: `new(api_base: String, api_key: Secret<String>, default_model: String)`

### `DegradationLevel` — 🔴 drift
**Doc:**
```rust
pub enum DegradationLevel {
    Healthy,
    Slow,
    FailingOver,
    Exhausted,
}
```

**Source** (`crates/providers/src/manager.rs`):
```rust
pub enum DegradationLevel {
    Fallback,
    Offline,
}
```

### Callback type signatures — 🔴 drift
| Type | Doc | Source |
|---|---|---|
| `OnCircuitOpen` | `Arc<dyn Fn(&str, &ProviderError)>` | `Arc<dyn Fn(Timestamp)>` |
| `OnProviderDegraded` | `Arc<dyn Fn(&str, DegradationLevel)>` | `Arc<dyn Fn(DegradationLevel)>` |

### Constants — 🔴 drift
| Constant | Doc | Source |
|---|---|---|
| `DEFAULT_CONTEXT_WINDOW` | `200_000` | `128_000` |

### `CacheAnchor` — 🔴 drift
**Doc:** `LastSystem`, `LastUser`, `LastTool`, `LastN(usize)`

**Source:** `LastSystem`, `LastTool`, `MessageIndex(usize)`

No `LastUser` or `LastN` variants exist.

### `ProviderCapabilities` — 🔴 major drift
**Doc fields:** `supports_tools`, `supports_streaming`, `supports_vision`, `supports_caching`, `supports_reasoning`, `supports_embeddings`, `supports_json_mode`, `max_context`, `max_output`

**Source fields** (`crates/providers/src/types.rs`): `extended_thinking`, `structured_outputs`, `prompt_caching`, `explicit_cache_markers`, `native_token_counting`, `vision`, `streaming`, `tool_choice_required`, `parallel_tool_calls`

Completely different field set.

### `ChatParams` — 🔴 drift
**Doc fields:** `model: String`, `temperature: f32`, `max_tokens: u32`, `tools: Option<Vec<Tool>>`, `tool_choice`, `response_format`, `cache_breakpoints: Vec<CacheBreakpoint>`, `cache_system_prompt: bool`, `stop_sequences: Vec<String>`, `reasoning_effort: Option<ReasoningEffort>`

**Source fields** (`crates/providers/src/types.rs`): `model: String`, `temperature: Option<f32>`, `max_tokens: Option<u32>`, `response_format: Option<ResponseFormat>`, `role: Option<ProviderRole>`, `session_key: Option<String>`

Different field set; temperature/max_tokens are `Option` in source but non-optional in doc.

### `ProviderHealth` — ⚠️ drift
**Doc:** `Healthy`, `Degraded { reason: String }`, `Unhealthy { reason: String, since: Timestamp }`

**Source:** `Healthy`, `Degraded(String)`, `Unhealthy(String)`, `Unknown`

Source adds `Unknown` variant and uses tuple variants instead of named fields.

### `id_implies_reasoning` — ✅ verified
Exists in source (`crates/providers/src/types.rs`) with matching signature.

---

## `storage`

### Module tree — ✅ verified
`pool.rs`, `repos/mod.rs` + 53 repo modules, `vector_store/`, `rows/`, `finance_storage.rs` all present.

### Repo count — ⚠️ cosmetic
Doc claims "~52 repos". Actual count:
- 53 `pub mod` declarations in `repos/mod.rs` (including `tests` which is not a repo → 52 repo modules)
- Wait: `grep -c "pub mod"` gave 54, but one is `tests`. Recount: 53 repo modules.
- `Repos` struct holds 37 fields, but `finance` is a `FinanceStorage` facade wrapping 9 finance repos.
- 8 additional repo modules exist outside `Repos`: `brain_signal`, `coaching_intervention_log`, `coaching_strategy`, `coding_background_jobs`, `reforge_suggestion`, `response_warning`, `retrieval_feedback`, `trial_repo`.
- Total distinct repo types ≈ 53.

Doc's "~52" is a reasonable approximation, but the exact count is **53** repo modules / types.

### `StoragePool` — ✅ verified
`connect()`, `from_existing()`, `connect_in_memory()`, `run_feature_migrations()` all match doc signatures and behavior (WAL mode, FK on, busy_timeout=5000ms, etc.).

### `FinanceStorage` — ✅ verified
Wraps 9 finance repos: accounts, transactions, budgets, investments, goals, liabilities, allocations, snapshots, exchange_rates. Matches doc.

---

## `tools-core`

### Module tree — ✅ verified
`approval_class.rs`, `feature.rs`, `interceptor.rs`, `job_supervisor.rs`, `metadata.rs`, `registry.rs`, `routing.rs`, `search.rs` all present.

### `Tool` trait — ✅ verified
Doc and source match on all methods: `name()`, `description()`, `parameters()`, `execute()`, `metadata()`, `is_concurrency_safe()`, `allowed_channels()`, `custom_timeout()`, `approval_class()`, `approval_scope()`, `to_schema()`, `validate_params()`.

### `ToolRegistry` — ✅ verified
`register`, `register_dyn`, `unregister`, `unregister_by_prefix`, `get`, `has`, `prepare`, `execute`, `get_definitions`, `record_usage`, `top_used`, `search_tools`, `take_all`, `tool_names`, `len`, `is_empty` all present and match doc.

### `ToolCallInterceptor` — 🔴 drift
**Doc documents:**
```rust
async fn before(&self, name: &str, args: &mut Value, ctx: &RoutingContext) -> Result<InterceptorOutcome>;
async fn after(&self, name: &str, result: &mut String, ctx: &RoutingContext) -> Result<()>;
```
with `InterceptorChain::run_before` and `run_after`.

**Actual source** (`crates/tools-core/src/interceptor.rs`):
```rust
#[async_trait]
pub trait ToolCallInterceptor: Send + Sync {
    async fn before_call(&self, tool_name: &str, args: &Value, skill_name: Option<&str>) -> Result<()>;
}

pub struct InterceptorChain { ... }
impl InterceptorChain {
    pub async fn check(&self, tool_name: &str, args: &Value, skill_name: Option<&str>) -> Result<()> { ... }
}
```

**Drifts:**
- Method name: `before` → `before_call`
- No `after` method exists
- No `InterceptorOutcome` enum exists
- `InterceptorChain` has `check` not `run_before`/`run_after`
- Arguments: doc has `args: &mut Value`, source has `args: &Value`; doc has `ctx: &RoutingContext`, source has `skill_name: Option<&str>`

### `RoutingContext` — ⚠️ minor drift
Doc and source fields broadly align. Source has these fields that doc either omits or names differently:
- `entity_tx: Option<mpsc::Sender<EntityCard>>` — not in doc
- `event_tx: Option<mpsc::Sender<ToolEvent>>` — not in doc
- `cancel_token: Option<CancellationToken>` — doc uses `cancel_token` (matches)
- `hook_engine: Option<Arc<HookEngine>>` — doc includes
- `job_supervisor: Option<DynJobSupervisor>` — doc includes

Doc's field listing is incomplete (missing `entity_tx`, `event_tx`) but not incorrect.

### `FeaturePackage` — ✅ verified
Matches exactly.

---

## TODO / FIXME / unimplemented! Inventory

**Result: ZERO actual TODO/FIXME/unimplemented! items found** in all 5 crates.

`grep -riE "\b(TODO|FIXME|XXX|HACK)\b"` returned only false positives (variable names like `coding_todo`, `todo_embeddings`, table names, etc.).

`grep -riE "unimplemented!\(|todo!\("` returned no matches.

**`#[allow(dead_code)]` markers found (5):**
1. `crates/desktop/src/focus_timer.rs` — 3 instances (likely debug/development helpers)
2. `crates/mcp/src/client/manager.rs` — 1 instance
3. `crates/storage/src/vector_store/mod.rs` — 1 instance
4. `crates/tools-core/src/registry.rs` — 1 instance

These indicate minor internal-only code that may be stale, but no active technical debt comments.

---

## Recommendations

1. **`providers` doc needs a full rewrite** of the `LlmProvider` trait, `ProviderCapabilities`, `ChatParams`, `CacheAnchor`, `DegradationLevel`, and adapter constructor sections. These are the most severe drifts.
2. **`mcp` doc** should move `start_health_check` from `McpCircuitBreaker` to `McpManager`, and replace `is_server_allowed` with `decide() -> AllowDecision`.
3. **`tools-core` doc** should remove `after`/`run_after` from `ToolCallInterceptor` and update `InterceptorChain` to document `check()` and `before_call()`.
4. **`desktop` doc** should correct `main()` and `run_desktop_app()` return types to `()`, and add a note about the 5th smoke test.
5. **`storage` doc** could update "~52 repos" to "53 repo modules" for precision.
6. **Cross-crate:** The `DynProvider = Arc<dyn LlmProvider>` change from `Box<dyn>` should be propagated to any other docs referencing it.
