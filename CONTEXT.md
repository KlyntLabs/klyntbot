# Klyntbot Domain Context

Klyntbot is a personal AI agent with a Rust backend, Tauri desktop frontend, and MCP server mode. It manages conversations, tasks, notes, productivity tracking, coaching interventions, and cognitive memory through an LLM-powered agent loop.

## Language

**FeaturePlugin**:
A trait in `tools-core` that defines tools, migrations, and health checks for a feature crate. Shallow — it does not cover lifecycle hooks like context sources or signal consumers.
_Avoid_: Feature package (too vague)

**AppCorePlugin**:
A trait for features that participate in full application initialization. Defines active registration hooks: `init()` for registering tools/context sources/consumers, and `post_init()` for post-assembly setup. Lives at the app-core seam, distinct from `FeaturePackage`.
_Avoid_: Extending `FeaturePackage` to cover lifecycle (would couple tool metadata to orchestration)

**FeatureHost**:
The deep module that owns the plugin lifecycle: resolving dependencies, running migrations, calling `init()` and `post_init()` in order, and providing a type-map for plugin handles. The sole place that knows about all installed plugins.
_Avoid_: Plugin registry (implies passive lookup), init container (too generic)

**PluginContext**:
The interface passed to `AppCorePlugin::init()`. Provides methods for active registration (`register_tool`, `add_context_source`, `add_signal_consumer`, etc.) and read-only access to shared dependencies.
_Avoid_: Plugin deps (too narrow — context is the registration surface, not just dependencies)

**PluginDeps**:
The bundle of shared infrastructure (storage pool, providers, config, buses, tokens) available to every plugin during initialization.
_Avoid_: Service locator (implies global mutable access), DI container (too framework-y)

**RoutingContext**:
The full bundle of channel/chat identity and optional per-call wiring (hooks, streaming channels, cancellation, interaction channels, job supervisor) threaded through every tool execution. It is the *transport-side* surface — the untyped `Tool::execute` boundary always receives the whole thing.
_Avoid_: Tool args (that's the JSON params), Tool deps (too narrow — it carries identity and runtime wiring, not just dependencies)

**Context view**:
A borrowed, zero-copy projection of `RoutingContext` exposing only the slice of fields a tool declares it needs (e.g. `HookCtx`, `IoCtx`). A tool's `ToolExecute::Ctx<'a>` associated type names its view; the `#[derive(Tool)]` bridge constructs it from the `RoutingContext` via `FromRoutingContext`.
_Avoid_: Sub-context (vague), Tool context (collides with the full `RoutingContext`)

**Context projection**:
The act of building a `Context view` from a `RoutingContext`, performed by the `Tool` bridge (`FromRoutingContext::project`). The seam that lets the wide transport struct serve narrow tool interfaces.
_Avoid_: Context conversion (implies ownership transfer; projection borrows)

**View ladder**:
The fixed superset chain of context views — `() ⊂ HookCtx ⊂ IoCtx ⊂ FullCtx`. A tool picks the smallest rung that fits; `FullCtx` (a `Deref` wrapper over `&RoutingContext`) is the escape hatch for the few genuinely-wide tools and the transitional view during migration.
_Avoid_: Context hierarchy (implies inheritance/nesting of values), context levels (ambiguous with delegation depth)

## Flagged ambiguities

- **"Feature"** is overloaded: it can mean a `feature-*` crate, a `FeaturePackage` impl, an `AiFeature` derive, or a user-facing capability. In this context, "feature" means the crate-level module; "plugin" means its participation in the host.
- **"Init"** previously meant the 1,750-line function in `app-core/src/init/mod.rs`. After the host extraction, "init" means the plugin's `init()` hook; the host orchestrates.

## Example dialogue

> **Dev:** I want to add a new `feature-reminders` crate.
>
> **Maintainer:** Great. Implement `AppCorePlugin` for it. In `init()`, register your `ReminderTool` via `ctx.register_tool()`. If you need a repo, build it from `ctx.deps.storage_pool` and stash the handle in `ctx.host` so commands can reach it later.
>
> **Dev:** Do I also need to touch `app-core/src/init/mod.rs`?
>
> **Maintainer:** No — that's the point of the host. Add your plugin to the host builder in `main.rs` (or `for_test()`). The host runs your migrations, calls `init()`, and wires everything. The only reason to touch `init/mod.rs` is if your plugin needs a dependency that isn't in `PluginDeps` yet.
