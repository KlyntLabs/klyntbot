# Subsystem 07 — Tools Framework

> **Status:** 🟢 Stable (wiring inconsistencies; ToolOutput::Structured unused in prod)
> **Status last verified:** 2026-05-16
> **Crates:** `tools-core`, `tools-core-macros`, `tools`
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

Three layers: **`tools-core`** holds the trait surface (`Tool`, `ToolExecute`, `ToolParams`, `FeaturePackage`, `ToolRegistry`, `ApprovalClass`, `RoutingContext`, `ToolOutput`, `FeatureMigration`, `JobSupervisorHandle`). **`tools-core-macros`** generates impl boilerplate from declarative attributes (5 macros). **`tools`** holds 14 concrete domain tool implementations plus embedding + fastembed + LanceDB integration.

The framework is solid. The drift comes from **4 different tool wiring paths** that evolved organically: most tools register via `FeaturePackage::tools()`, but `TaskTool`, `AlarmTool`, `LearningTool` are wired in `agent_loop::builder`; `LauncherTool` is wired in `app-core::init`; `AgentTaskTool` is *cloned per subagent invocation*. Anyone adding a tool today has to read the source to figure out which path applies.

There is **no `ConcurrencyClass`** on the `Tool` trait. Concurrency is a single `is_concurrency_safe(args) -> bool`. The bus crate defines a `ConcurrencyClass` enum but the `Tool` trait doesn't use it.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef tr fill:#fff9c4,stroke:#f9a825,color:#f57f17
    classDef mc fill:#fff8e1,stroke:#f57f17,color:#e65100
    classDef tl fill:#fffde7,stroke:#fbc02d,color:#f57f17
    classDef reg fill:#fff3e0,stroke:#fb8c00,color:#e65100
    classDef ext fill:#fff,stroke:#999,stroke-dasharray:5

    T[Tool trait<br/><i>name · description · parameters · execute<br/>metadata · approval_class · allowed_channels<br/>is_concurrency_safe · custom_timeout</i>]:::tr
    TE[ToolExecute trait<br/><i>typed Params + execute<br/>used with #[derive(Tool)]</i>]:::tr
    TP[ToolParams trait<br/><i>json_schema + from_args</i>]:::tr
    FP[FeaturePackage trait<br/><i>name · tools · migrations · health_check</i>]:::tr
    APP[ApprovalClass<br/><i>Safe · Sensitive · Destructive · Admin</i>]:::tr
    RC[RoutingContext<br/><i>22+ fields: channel · session_mode<br/>workspace_cwd · agent_chain · ...</i>]:::tr
    JS[JobSupervisorHandle trait<br/><i>spawn · output_delta · stop · list<br/>+ PTY: write_stdin · resize · attach · detach</i>]:::tr
    TO[ToolOutput<br/><i>Text(String)<br/>Structured { summary, data } — UNUSED</i>]:::tr

    M1[#[derive(Tool)]]:::mc
    M2[#[derive(ToolParams)]]:::mc
    M3[#[derive(ActionParams)]]:::mc
    M4[#[derive(DomainEnum)]]:::mc
    M5[#[tool_actions(...)]]:::mc

    REG[ToolRegistry<br/><i>register · get · prepare · execute<br/>get_definitions · usage tracking</i>]:::reg
    IC[InterceptorChain<br/><i>pre-execution middleware</i>]:::reg

    TOOLS[14 domain tools<br/><i>memory · okr · area · project · cron<br/>annotate · mirror · subagents · temporal · docs<br/>learning · context_request · skill_reference · agent_task</i>]:::tl
    EMB[fastembed + LanceDB<br/><i>EmbeddingEngineImpl · EmbeddingStore<br/>EMBEDDING_DIM = 384</i>]:::tl

    APP --> T
    RC --> T
    M1 --> TE
    M2 --> TP
    M3 --> M5
    M5 --> T
    TE --> T
    T --> REG
    IC --> REG
    TOOLS --> T
    EMB --> TOOLS

    EXT_AG[agent::ExecutionCore]:::ext
    EXT_MCP[mcp server]:::ext
    EXT_GATE[approval::ApprovalGate]:::ext
    REG --> EXT_AG
    REG --> EXT_MCP
    APP --> EXT_GATE
```

---

## Mental model

The framework's core abstraction is the **`Tool` trait**. It's an async, untyped JSON-in / String-out interface: `execute(args: Value, ctx: &RoutingContext) -> Result<String>`. The interesting design decisions are:

1. **Two-tier macros.** Tool authors write a `Params` struct + a `ToolExecute` impl. `#[derive(Tool)]` bridges the typed `ToolExecute` to the untyped `Tool`. The author never writes JSON parsing or schema generation themselves.
2. **One untyped result type.** `execute()` returns `Result<String>`. Structured returns piggy-back on a `__STRUCTURED__` prefix that `ToolOutput::parse()` recognizes. The richer `ToolOutput::Structured { summary, data }` variant exists but **no tool in production emits it.**
3. **Approval is a per-call decision.** `tool.approval_class(&args)` is called with the actual arguments — so `MemoryTool` can return `Destructive` for `purge` but `Safe` for `search`. `tool.approval_scope(&args)` can return a per-resource key (e.g., file path) so grants can be scoped: "always allow edit on `src/main.rs`."
4. **`RoutingContext` is wide on purpose.** 22+ fields. It carries everything a tool might need: channel, session mode, cancel token, interaction channel for prompts, workspace cwd, agent chain, plan-mode state, job supervisor. Adding a field doesn't break tools; ignored fields are zero-cost.

### Concurrency is a method, not an enum

The bus crate defines `ConcurrencyClass { Safe, Sequential, Exclusive }`. **The `Tool` trait does not use it.** Instead:

```rust
fn is_concurrency_safe(&self, _args: &Value) -> bool { false }
```

The execution loop partitions tool calls into:
- **Safe** (`is_concurrency_safe → true`): execute via `join_all` up to `MAX_CONCURRENT_TOOLS = 10`.
- **Unsafe** (default, `false`): execute sequentially.

There is no third tier (no `Exclusive`). If a tool needs serial-only execution against a particular resource, it must encode that logic itself (e.g., via an internal mutex).

---

## Reference

### `tools-core` — file map

| Path | Purpose |
|---|---|
| `src/lib.rs` | `Tool`, `ToolExecute`, `ToolParams` traits; `ToolOutput`; barrel |
| `src/approval_class.rs` | `ApprovalClass`, `ApprovalScope` |
| `src/config_persistence.rs` | `ConfigPersistence` trait for tools that persist config to DB |
| `src/events.rs` | `ToolEvent` — emitted per-call to Tauri/WS relay |
| `src/feature.rs` | `FeaturePackage`, `FeatureMigration`, `HealthStatus` |
| `src/interceptor.rs` | `ToolCallInterceptor`, `InterceptorChain` (pre-execution middleware) |
| `src/job_supervisor.rs` | `JobSupervisorHandle`, `JobSpec`, `JobView`, `JobId`, `RingRead`, PTY dimension constants |
| `src/metadata.rs` | `ToolMetadata`, `ToolCategory`, `CostHint`, `ToolSource` |
| `src/pagination.rs` | `Page<T>` |
| `src/params.rs` | `ParamExtractor` convenience for raw JSON argument parsing |
| `src/registry.rs` | `ToolRegistry` — lookup, execute, schema caching, usage tracking |
| `src/routing.rs` | `RoutingContext`, `ProgressHandler`, `InteractionChannel`, `InteractionBundle` |
| `src/search.rs` | `Searchable` trait; `rrf_merge` / `rrf_merge_triple` for Reciprocal Rank Fusion |
| `src/validation.rs` | Internal JSON Schema validator used by `ToolRegistry::prepare` |

### `Tool` trait — the canonical surface

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;                                            // JSON Schema object
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;

    fn metadata(&self) -> ToolMetadata                  { ToolMetadata::default() }
    fn is_concurrency_safe(&self, _args: &Value) -> bool { false }
    fn allowed_channels(&self) -> common::ChannelMask    { common::ChannelMask::ALL }
    fn custom_timeout(&self) -> Option<Duration>         { None }
    fn approval_class(&self, _args: &Value) -> ApprovalClass { ApprovalClass::Safe }
    fn approval_scope(&self, _args: &Value) -> ApprovalScope { ApprovalScope::ToolAction }
    fn to_schema(&self) -> Value                         { /* OpenAI function-calling fmt */ }
    fn validate_params(&self, params: &Value) -> Vec<String>;
}
pub type DynTool = Arc<dyn Tool>;
```

### `ApprovalClass` / `ApprovalScope`

```rust
pub enum ApprovalClass { Safe, Sensitive, Destructive, Admin }
pub enum ApprovalScope {
    ToolAction,
    ToolActionResource(String),       // e.g., file path → grants can be per-resource
}
```

`requires_prompt_on_remote()` returns `true` only for `Destructive | Admin`. `Sensitive` typically auto-allows on remote channels (the local desktop user opted into the channel; sensitive ops are already implicit).

### `RoutingContext` fields (the wide one)

`channel`, `session_mode`, `chat_id`, `interaction_tx`, `is_direct_mode`, `delegation_depth`, `entity_tx`, `interaction_channel`, `champion_params` (autotuner override), `cancel_token`, `event_tx`, `hook_engine`, `session_key`, `message_id`, `repo_id`, `agent_id`, `agent_profile`, `plan_mode_active`, `plan_session_id`, `previous_anti_passivity_violation`, `same_turn_user_msg_emitted`, `workspace_cwd`, `agent_chain`, `job_supervisor`.

Tools ignore fields they don't need; constructors fill `Option`s with `None`.

### `ToolOutput` (and the unused convention)

```rust
pub enum ToolOutput {
    Text(String),
    Structured { summary: String, data: serde_json::Value },
}
```

`Structured` is parsable via the `__STRUCTURED__` prefix convention. **No tool in production emits this prefix.** Forward-compatibility seam. See [`TECH_DEBT.md`](../TECH_DEBT.md) §8.

### `FeaturePackage` trait

```rust
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<DynTool>;
    fn migrations(&self) -> Vec<FeatureMigration>;
    async fn health_check(&self) -> Result<HealthStatus> { Ok(HealthStatus::Healthy) }
}
```

### `FeatureMigration`

```rust
pub struct FeatureMigration {
    pub feature_name: String,
    pub version: i64,
    pub description: String,
    pub sql: String,
}
```

### `JobSupervisorHandle` trait (the substantial PTY-aware surface)

```rust
#[async_trait]
pub trait JobSupervisorHandle: Send + Sync + Debug {
    async fn spawn(&self, spec: JobSpec) -> Result<JobView, JobError>;
    async fn output_delta(&self, id: &JobId, since: u64, block: bool, timeout_ms: u64) -> Result<RingRead, JobError>;
    async fn stop(&self, id: &JobId, reason: &str) -> Result<JobView, JobError>;
    async fn list(&self, session_id: &str, agent_chain: &[String], active_only: bool) -> Vec<JobView>;

    // PTY extensions (default → Err(JobError::NotPty)):
    async fn write_stdin(&self, id: &JobId, data: &[u8]) -> Result<usize, JobError>;
    async fn resize(&self, id: &JobId, rows: u16, cols: u16) -> Result<(), JobError>;
    async fn attach(&self, id: &JobId) -> Result<AttachHandle, AttachError>;
    async fn detach(&self, id: &JobId) -> Result<(), AttachError>;
    async fn set_attach_channel(&self, id: &JobId, tx: UnboundedSender<Vec<u8>>) -> Result<(), AttachError>;
}
pub type DynJobSupervisor = Arc<dyn JobSupervisorHandle>;
```

Concrete implementation lives in `feature-coding-bash`. Tools reach it via `RoutingContext::job_supervisor`.

### `ToolRegistry` API (selected)

```rust
impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register<T: Tool + 'static>(&mut self, t: T);
    pub fn register_dyn(&mut self, t: DynTool);
    pub fn unregister(&mut self, name: &str);
    pub fn unregister_by_prefix(&mut self, prefix: &str) -> usize;

    pub fn get(&self, name: &str) -> Option<DynTool>;
    pub fn has(&self, name: &str) -> bool;
    pub fn prepare(&self, name: &str, args: &Value, ctx: &RoutingContext) -> Result<DynTool>;
    pub async fn execute(&self, name: &str, args: Value, ctx: &RoutingContext) -> Result<String>;

    pub fn get_definitions(&self) -> Arc<Vec<Value>>;     // cached schema list for the LLM
    pub fn get_metadata(&self, name: &str) -> Option<&ToolMetadata>;
    pub fn list_meta(&self) -> Vec<&ToolMetadata>;
    pub fn by_category(&self, cat: &ToolCategory) -> Vec<DynTool>;

    pub fn record_usage(&self, name: &str);
    pub fn top_used(&self, n: usize) -> Vec<(String, u64)>;
    pub fn take_all(&mut self) -> Vec<DynTool>;
    pub fn tool_names(&self) -> Vec<String>;
}
```

### `tools-core-macros` — what each macro generates

| Macro | Requires | Generates |
|---|---|---|
| `#[derive(Tool)]` | `#[tool(name, description, params = "T")]` + `impl ToolExecute` | Full `impl Tool` that delegates `parameters()` → `T::json_schema()` and `execute()` → `T::from_args` + `ToolExecute::execute`. Optional attrs: `concurrency_safe`, `allowed_channels` (`all`/`coding_only`/`desktop_only`/`non_coding`), `custom_timeout_secs`, `approval_class` (`safe`/`sensitive`/`destructive`/`admin`), `approval_scope` (field name → resource key) |
| `#[derive(ToolParams)]` | Struct fields | `impl ToolParams` with `json_schema()` (using doc-comments for `description`) and `from_args(args)`. Per-field: `#[param(required, min=N, max=N, min_length=N, max_length=N)]` |
| `#[derive(ActionParams)]` | Struct fields | **Inherent methods** (not trait impl): `pub fn json_schema()`, `pub fn from_value(args: &Value)`. Used by per-action structs with `#[tool_actions]` |
| `#[tool_actions(...)]` | `impl` block; methods tagged `#[action(name = "...")]` | Full `impl Tool` with merged schema (union of all action params + an `action` enum field) and dispatch by reading `args["action"]`. Warns at compile time on conflicting field types across actions |
| `#[derive(DomainEnum)]` | Unit-variant enum | `as_str()` (PascalCase → snake_case; `#[canonical("custom")]` overrides), `from_str_loose()` (case-insensitive with `#[aliases(...)]`), `impl Display`, `impl FromStr` |

### JSON Schema generation (`classify_type` in `tools-core-macros/src/helpers.rs`)

| Rust type | JSON type |
|---|---|
| `String` | `"string"` |
| `bool` | `"boolean"` |
| `u8`..`u64`, `i8`..`i64` | `"integer"` |
| `f32`, `f64` | `"number"` |
| `Vec<T>` (T primitive) | `"array"` with `items.type` |
| `Option<T>` (T primitive) | same type, optional (no `required` entry) |

**Nested structs panic at compile time** with: `"unsupported field type — wrap it: pub field: String and convert in your handler"`.

### Domain tool inventory (the 14)

| Struct | Registry key | Purpose | Multi-action | ApprovalClass | AllowedChannels |
|---|---|---|---|---|---|
| `MemoryTool` | `memory` | Semantic fact storage: search/record/purge | No | `record_fact` → Sensitive; `purge` → Destructive; else Safe | ALL |
| `OkrTool` | `okr` | OKR hierarchy CRUD | No | Deletes Destructive; writes Sensitive; else Safe | ALL |
| `AreaTool` | `area` | PARA area CRUD | No | Safe | ALL |
| `ProjectTool` | `project` | Project container CRUD | No | Safe | ALL |
| `CronTool` | `cron` | Cron job management | No | add/remove/enable/disable/run → Destructive; else Safe | ALL |
| `LearningTool` | `learning` | Tool-outcome learning status | No | Safe | NON_CODING |
| `AnnotateTool` | `annotate` | Annotation CRUD + search | **Yes** | Safe | ALL |
| `MirrorTool` | `mirror` | Mirror introspection | **Yes** | Safe | ALL |
| `SubagentsTool` | `subagents` | Subagent spawn/resume/list/kill | **Yes** | Safe | ALL |
| `TemporalTool` | `temporal` | Temporal fact recall | **Yes** | Safe | ALL |
| `DocsTool` | `docs` | Content registry search/get/list | **Yes** | Safe | ALL |
| `ContextRequestTool` | `context_request` | Mid-execution context expansion | No | Safe | ALL |
| `SkillReferenceTool` | `skill_reference` | Load full skill body by name | No | Safe | ALL |
| `AgentTaskTool` | `agent_task` | Per-invocation task claim/complete/fail (subagents) | No | claim/complete/fail → Sensitive | ALL |

Other tools wired alongside (NOT in `tools` crate):
- `TaskTool` (`tasks`) — in `feature-tasks`
- `AlarmTool` (`alarm`) — in `feature-alarms`

---

## The four wiring paths

CLAUDE.md says `TaskTool` is "the exception" wired in the agent builder. **Actually there are 4 paths**, each used by different tools, with no project-level documentation explaining why.

### Path A — `FeaturePackage::tools()` (most common)

A feature crate impls `FeaturePackage`; its `tools()` returns `Vec<DynTool>`. The agent builder iterates and calls `registry.register_dyn(tool)`.

Used by: nearly every `feature-*` crate.

**When to use:** the tool's dependencies are known at feature-construction time and the tool takes no per-call state.

### Path B — `crates/agent/src/agent_loop/builder.rs`

Three tools live here:

- **`AlarmTool`** (`builder.rs:690`) — `feature-alarms` exports only the tool struct, not a `FeaturePackage` impl. Wired manually: `tool_registry.register(alarm_tool)`.
- **`TaskTool`** (`builder.rs:1353` → 1417) — Constructed with many injected deps (progress handler, embedding handler, domain bus, alarm writer), wrapped in `TasksFeature::with_task_tool(...)`, then registered via the feature.
- **`LearningTool`** (`builder.rs:1735`) — Registered directly; the actual `LearningTool` lives in `crates/tools/src/domain/learning_tool.rs`. `feature-learning::FeaturePackage::tools()` returns `vec![]` deliberately.

**When to use:** the tool needs runtime-constructed dependencies that a feature crate's constructor can't hold.

### Path C — `crates/app-core/src/init/mod.rs:1132`

One tool: **`LauncherTool`**.

`LauncherFeature::with_tool_deps(...)` is called from app-core init because the launcher engine is an app-core concept (holds launcher registry, frequency repo, pins repo). The agent builder has no access to it. Wiring happens post-agent-construction: `agent.tool_registry().write().await` → `registry.register_dyn(tool)`.

**When to use:** the tool depends on state that lives in `app-core`, not in the agent builder.

### Path D — `crates/agent/src/subagent.rs:800,819`

One tool: **`AgentTaskTool`** — the only per-invocation tool.

The subagent runner clones the cached base registry and appends a fresh `AgentTaskTool` bound to this invocation's task claim. Per `run_subagent_task` call.

**When to use:** the tool needs per-call state (e.g., a specific task-claim ID) that a singleton can't carry.

---

## Workflows

### A tool call from the LLM (end-to-end)

```
1. LLM emits {name, args}
   ↓
2. agent/src/execution/core.rs:820-868 (preflight):
   - read-lock the registry
   - prepare(name, args, ctx) — validates params, returns DynTool
   ↓
3. If InterceptorChain exists, run pre-execution hooks (e.g., MCP wrappers)
   ↓
4. If ApprovalGate exists:
   - class = tool.approval_class(&args)
   - scope = tool.approval_scope(&args)
   - ClassifyHooks run (last non-None override wins — CodingApprovalPolicy hooks here)
   - ApprovalClass::Safe → Allow immediately
   - Remote channel without capability for class → Allow (channel opt-in is sufficient)
   - Existing grant (session-scoped or session-key-scoped) → Allow
   - Otherwise → prompt via ApprovalChannel; block until user responds OR cancel_token fires
   ↓
5. GateOutcome:
   - Allow → execute
   - Deny → PermissionDenied error
   - Cancel → Cancelled error
   ↓
6. tool.execute(args, ctx) — runs to completion
   ↓
7. ToolOutput::parse(result):
   - Starts with __STRUCTURED__ → ToolOutput::Structured { summary, data }
   - Otherwise → ToolOutput::Text(result)
```

### Adding a tool via `#[derive(Tool)]`

```rust
#[derive(ToolParams)]
struct MyParams {
    #[param(required)]
    query: String,
    #[param(min = 1, max = 100)]
    limit: Option<u32>,
}

#[derive(Tool)]
#[tool(
    name = "my_tool",
    description = "Search the thing",
    params = "MyParams",
    concurrency_safe = "true",
    allowed_channels = "non_coding",
    approval_class = "safe",
)]
struct MyTool { /* deps */ }

#[async_trait]
impl ToolExecute for MyTool {
    type Params = MyParams;
    async fn execute(&self, params: MyParams, ctx: &RoutingContext) -> Result<String> {
        // do the thing
        Ok("ok".into())
    }
}
```

### Multi-action tool via `#[tool_actions]`

```rust
#[derive(ActionParams)]
struct SearchParams { #[param(required)] query: String }
#[derive(ActionParams)]
struct AddParams { #[param(required)] title: String }

struct MyTool { /* deps */ }

#[tool_actions(
    name = "my_tool",
    description = "Search or add things",
    category = "productivity",
)]
impl MyTool {
    #[action(name = "search")]
    async fn search(&self, p: SearchParams, ctx: &RoutingContext) -> Result<String> { ... }

    #[action(name = "add")]
    async fn add(&self, p: AddParams, ctx: &RoutingContext) -> Result<String> { ... }
}
```

The macro generates a merged schema with an `action` enum field; dispatch happens by reading `args["action"]`.

---

## Internals

### `prepare` is the validation gate

`ToolRegistry::prepare(name, args, ctx)` runs:
1. `tool = registry.get(name).ok_or(ToolNotFound)?`
2. `errors = tool.validate_params(args)`; if non-empty → `InvalidParams(errors.join("; "))`
3. Returns the `DynTool` for the caller to execute.

This is run-time validation against the JSON Schema. Schema generation from `ToolParams` is best-effort — `validate_params` is the authoritative gate.

### Approval per-call, not per-tool

`tool.approval_class(&args)` reads the actual arguments. `MemoryTool` returns:
- `record_fact` action → `Sensitive`
- `purge` action → `Destructive`
- Other actions → `Safe`

That's why the gate is called per-call, not per-tool registration.

### `CodingApprovalPolicy` is a `ClassifyHook`

In coding mode, `CodingApprovalPolicy` is registered as a `ClassifyHook` on the `ApprovalGate`. Variants:

- **`Default`** — compiled glob-based allow/deny/ask rules from `CodingPermissions` config.
- **`PlanMode`** — restricts write ops to the plan file only.
- **`YoloMode`** — time-bounded full auto-allow.

The hook's `classify()` can **upgrade or downgrade** the tool's declared `approval_class` based on the file-path argument. E.g., `read` to `/src/main.rs` may be auto-allowed while `read` to `/.ssh/` becomes `Sensitive`.

### Tool definitions are cached

`get_definitions()` returns `Arc<Vec<Value>>`. Built on first call after any `register_*` invocation; mutating ops invalidate the cache. The agent runtime calls this once per LLM call.

### Concurrency partitioning in the execution loop

`agent::execution::core` partitions a batch of tool calls into:
- Safe (collected, sent through `join_all` capped at `MAX_CONCURRENT_TOOLS = 10`)
- Unsafe (executed sequentially in original order)

A safe tool with unsafe args doesn't exist — `is_concurrency_safe(args)` takes the args. Most tools just return a constant `true` or `false`.

### `__STRUCTURED__` is parsed but never produced

The parser exists; no tool emits the prefix. The forward-compatibility seam was added with intent to migrate certain tools (e.g., `tasks` query results → structured data). The migration never happened.

### Plural / singular footgun for MCP exposure

Registry keys are `tasks`, `notes` (plural) but `memory`, `finance`, `alarm` (singular). MCP exposure inherits these names: `mcp__klyntbot__tasks` (works), `mcp__klyntbot__task` (404). Standardize at the next MCP whitelist refresh.

---

## Dependencies & extension points

### Upstream deps

- `common` (errors, ChannelMask, SessionMode, EntityCard, TrialParams)
- `tokio` (async runtime)
- `serde` + `serde_json` (params, schema)
- `async-trait`
- `klynt-hooks` (HookEngine — passed through RoutingContext)
- (tools crate only) `fastembed`, `lance`, `lancedb`

### Adding a new tool

**First decision: where does the tool live?**

| Tool type | Lives in | Examples |
|---|---|---|
| Cross-cutting "core" tool (memory, OKR, annotate, mirror, …) | `crates/tools/src/domain/<my_tool>.rs` | `MemoryTool`, `MirrorTool`, `OkrTool` |
| Feature-bound tool (tasks, notes, finance, productivity, …) | A `crates/feature-<my_feature>/` crate | `TaskTool` (feature-tasks), `NotesTool` (feature-notes) |
| Coding-mode primitive (bash, read, edit, …) | `crates/klynt-core/src/tools/<my_tool>.rs` | `BashTool`, `ReadTool` |

**Rule of thumb:** if the tool has substantial domain state (its own DB tables, its own background services), it belongs in a `feature-*` crate. If it's a small utility consumed across domains, it belongs in `crates/tools/`. If it's a coding-mode primitive that the LLM uses to manipulate the workspace, it belongs in `klynt-core`.

**Then:**

1. Choose wiring path (A/B/C/D — see [above](#the-four-wiring-paths)). Default to A.
2. Implement `ToolParams` + `ToolExecute` (single-action) or `ActionParams` + `#[tool_actions]` (multi-action).
3. Declare `approval_class` and `allowed_channels`. `allowed_channels` is what gates the LLM's tool visibility per session — the agent runtime filters the tool registry per-turn by the active session's channel.
4. Wire via the chosen path. For Path A, registration happens in `crates/agent/src/agent_loop/builder.rs` which iterates `feature.tools()` and calls `registry.register_dyn(tool)` for each.
5. (Optional) Add registry name to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs` to expose via MCP. **Watch for plural/singular mismatch.**
6. (Optional) Add migrations via `FeaturePackage::migrations()` or a new built-in migration.
7. Verify: `cargo nextest run -p klyntbot-server` to confirm MCP advertises it.

### Adding a new `ApprovalClass` variant

⚠️ Cross-cutting. Every gate hook (`CodingApprovalPolicy`, plugin gates, MCP server gates) must handle it. The `requires_prompt_on_remote()` logic needs updating. Coordinate carefully.

### Adding a `JobSupervisorHandle` implementation

Implement the trait; pass an `Arc<MyImpl>` into `RoutingContext::job_supervisor` at construction. Today only `feature-coding-bash` provides a concrete impl.

---

## Open questions & debt

- **Tool wiring has 4 different paths** — should be normalized OR documented in CLAUDE.md (this doc is the first place it's fully spelled out).
- **`ToolOutput::Structured` is defined but never produced** in production. Decide: implement, or remove.
- **`ConcurrencyClass` enum in `bus`** is not used by `Tool`. Either wire it up (third "Exclusive" tier?) or remove it.
- **Plural/singular tool-name footgun** (`tasks`/`notes` vs `memory`/`finance`/`alarm`). Pick one convention.
- **`feature-alarms`, `feature-learning`, `feature-insights`** don't follow the `FeaturePackage` pattern (no impl / empty impl / no impl + no tools). Misleading naming.
- **`TasksFeature::new()` without `.with_task_tool(...)` silently registers zero tools.** Footgun for plugins/tests.
- **JSON Schema generation doesn't support nested structs** — panics at compile time. The error is descriptive; the limitation should be acknowledged in the macro docs.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #7 (architectural anomalies), #8 (naming) for specifics.

---

## Cross-references

- [`01-foundations.md`](./01-foundations.md) — `common::ChannelMask`, `common::Result`
- [`02-storage.md`](./02-storage.md) — `FeatureMigration` consumed there
- [`04-agent-runtime.md`](./04-agent-runtime.md) — `ToolRegistry` lives in agent loop; `RoutingContext` constructed per turn
- [`08-assistant-features.md`](./08-assistant-features.md) — every `feature-*` crate consumes this framework
- [`09-coding-mode.md`](./09-coding-mode.md) — `JobSupervisorHandle` concrete impl
- [`10-sandboxing-security.md`](./10-sandboxing-security.md) — `ApprovalGate` consumes `ApprovalClass`
- [`crates/tools-core.md`](../crates/tools-core.md) — *(planned)* method-level reference
