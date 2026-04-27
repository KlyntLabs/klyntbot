# Typed Command Macros Design Spec

**Status:** Brainstormed and approved; ready for implementation plan.
**Date:** 2026-04-27.
**Related plans:** Builds on Plan 5 (typed Tauri IPC via tauri-specta) and the cleanup-quick-wins sweep (`CommandResult<T>` alias). Both must land first.

---

## Goal

Replace 465 hand-annotated `#[tauri::command]`/`#[specta::specta]` command shells with two purpose-built attribute macros — `#[klynt_command]` for the happy path and `#[klynt_raw_command]` for outliers — so the IPC layer becomes:

- **Concise:** the happy-path macro collapses ~5 lines of ceremony per command into one attribute.
- **Provably exhaustive on the runtime side:** every macro invocation pushes one entry into a `linkme::distributed_slice`. The slice IS the truth for what's runtime-dispatchable; you cannot accidentally write a Tauri command without registering it.
- **Drift-checked on the FE-binding side:** a 50ms test asserts the runtime slice and the specta `collect_commands![...]` hand-list contain the same names. Forgetting one fails CI with an exact "missing/extra" report.
- **Bulletproof on misuse:** the happy-path macro hard-refuses any signature deviation with span-aware compile errors. There is no permissive "smart-detect" path.

## Non-goals

- **Codegen-from-AppCore.** The macro never generates the function body. AppCore method renames produce normal `cargo build` errors with line numbers, not cryptic macro errors.
- **Auto error-conversion.** AppCore handlers already return `Result<T, ApiError>`; the macro doesn't convert errors and doesn't ship a `From<KlyntbotError> for ApiError` impl.
- **Replacing tauri-specta's TS-binding generation.** Specta keeps its compile-time `collect_commands![...]` for FE binding production. We work *with* its constraints, not around them.
- **Migrating the 50 internal `agent:*` events.** This is purely the *command* surface.
- **Build-time codegen.** No `build.rs` parsing of source files. Plan 6 stays at the proc-macro layer.

## Background

Today the IPC surface has three sources of truth that must stay in sync:

1. The function's `#[tauri::command]` annotation — establishes the command exists.
2. The per-module `pub(crate) const DEV_COMMANDS: &[&str] = &[...]` array — used by the dev-server HTTP fallback.
3. The `tauri::generate_handler![...]` (or post-Plan 5, `tauri_specta::collect_commands![...]`) registration list — used by the runtime invoke handler and the FE TS binding generator.

Two coverage tests (`dev_server_covers_all_tauri_commands`, `dev_server_has_no_orphan_commands`) exist *because* these three sources hand-drift. CLAUDE.md flags this as a gotcha.

The proposed macros collapse the first two sources into one (the macro emits both attributes), and the third becomes drift-checked against the first.

## Architecture

### Crate layout

A new crate `crates/desktop-macros/` at workspace layer L7 (sibling of `desktop-shared` and `desktop`).

Why a new crate vs extending `tools-core-macros`:

- `tools-core-macros` lives at L1 and is consumed by L1–L6 crates. Putting Tauri-specific macros there forces L1 crates to know about L7 concepts — a layer inversion forbidden by CLAUDE.md's dependency rule.
- The new macros emit references to `tauri::State`, `linkme`, and specta types — concepts that don't exist below L7.
- Single-purpose proc-macro crates compile faster.

### Dependency graph

```
crates/desktop-macros (proc-macro = true)
  └── proc_macro2, syn, quote                  # standard proc-macro tooling

crates/desktop-shared
  └── (unchanged — already exports CommandResult, ApiError, EntityKind, ...)

crates/desktop
  ├── desktop-macros                            # new
  ├── linkme = "0.3"                            # new — runtime registry crate
  ├── tauri-specta, specta, specta-typescript   # from Plan 5
  └── (everything else)
```

`linkme` lives in the desktop crate's `Cargo.toml` only — never workspace-wide. It's a registration concern. `desktop-macros` itself does not depend on `linkme`; it just emits the attribute path (the same way `serde_derive` emits `serde::*` paths without depending on `serde`).

### The distributed slice

Declared once in `crates/desktop/src/specta_builder.rs`:

```rust
use linkme::distributed_slice;

#[distributed_slice]
pub static KLYNT_COMMANDS: [CommandRegistration] = [..];

pub struct CommandRegistration {
    pub name: &'static str,
    pub invoke: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool,
    pub source: SourceKind,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SourceKind {
    Klynt,        // emitted by #[klynt_command]
    Raw,          // emitted by #[klynt_raw_command]
}
```

Co-located with its consumer (`build_specta()`), keeping the producer-of-truth and consumer-of-truth one hop apart. `desktop-shared` stays a pure types module.

---

## Macro 1 — `#[klynt_command]` (happy path)

### Contract

A function MAY be annotated with `#[klynt_command]` if and only if:

- Function is `pub async fn`.
- Function does NOT declare a `state` parameter (macro injects it).
- Function returns a bare type `T` — NOT `Result<T, _>`, NOT `CommandResult<T>`. The `()` return type is allowed.
- Function intends to use `State<'_, Arc<AppCore>>` (auto-injected by the macro).

A function MAY declare `app: tauri::AppHandle` as any parameter — the macro passes it through. Same for any number of typed argument parameters.

### Input the developer writes

```rust
#[klynt_command]
pub async fn task_get(id: String) -> Option<TaskResponse> {
    state.task_get(id).await
}
```

### What the macro expands to

```rust
#[tauri::command]
#[specta::specta]
pub async fn task_get(
    state: ::tauri::State<'_, ::std::sync::Arc<crate::app_core::AppCore>>,
    id: String,
) -> ::desktop_shared::CommandResult<Option<TaskResponse>> {
    state.task_get(id).await
}

#[::linkme::distributed_slice(crate::specta_builder::KLYNT_COMMANDS)]
#[allow(non_upper_case_globals)]
static __klynt_command_task_get: crate::specta_builder::CommandRegistration =
    crate::specta_builder::CommandRegistration {
        name: "task_get",
        invoke: __cmd__task_get,    // tauri-internal wrapper from #[tauri::command]
        source: crate::specta_builder::SourceKind::Klynt,
    };
```

The `__cmd__<name>` wrapper is a stable Tauri 2.x convention. Plan 6's first task verifies this on the pinned Tauri version (see "Pre-implementation verifications" below).

### Failure modes

| Misuse | Compile error (span-aware) |
|---|---|
| `pub fn task_get(...)` (missing `async`) | `klynt_command requires \`pub async fn\`` — span on `fn` |
| `pub async fn task_get(state: State<...>, ...)` | `klynt_command injects \`state\` automatically — remove this parameter` — span on the `state` arg |
| `pub async fn task_get(...) -> Result<T, ApiError>` | `klynt_command wraps return type for you — declare bare \`T\`` — span on the return type |
| `pub async fn task_get(...) -> CommandResult<T>` | Same message — span on the return type |
| `async fn task_get(...) -> T` (not `pub`) | `klynt_command requires \`pub\`` — span on `fn` |
| `pub async fn task_get(...)` (no return type) | `klynt_command requires an explicit return type` — span on the function signature |
| `#[klynt_command] struct Foo;` | `klynt_command can only be applied to functions` — span on the item |

All errors use `syn::Error::new_spanned` so IDE squiggles land on the offending token, not the macro invocation site.

### Macro takes no arguments

`#[klynt_command]` exactly. Never `#[klynt_command(...)]`. Any flexibility (state shape, sync, rename_all, no-state) lives in `#[klynt_raw_command]`. Bright line: zero args = canonical IPC shape.

---

## Macro 2 — `#[klynt_raw_command]` (escape hatch)

### Contract

`#[klynt_raw_command]` does one thing: register the command into `KLYNT_COMMANDS`. It does NOT inject `state`, wrap return types, emit `#[tauri::command]`, or emit `#[specta::specta]`. The developer keeps every Tauri/Specta knob; the macro just makes sure the command is in the slice.

### Input the developer writes

```rust
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub fn resize_window(app: tauri::AppHandle, label: String, height: f64) {
    let _ = app.get_webview_window(&label).map(|w| w.set_size(/* ... */));
}
```

Or for `rename_all` outliers (5 finance + 21 productivity):

```rust
#[klynt_raw_command]
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn finance_allocation_target_upsert(
    state: tauri::State<'_, std::sync::Arc<AppCore>>,
    accountId: String,
    targetPct: f64,
) -> CommandResult<()> { state.finance_allocation_target_upsert(accountId, targetPct).await }
```

Or for FocusTimer-state commands:

```rust
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn distraction_allow_temp(
    timer: tauri::State<'_, std::sync::Arc<FocusTimer>>,
    minutes: u32,
) -> CommandResult<()> { timer.allow_temp(minutes).await }
```

### What the macro expands to

```rust
// Function emitted unchanged.
#[tauri::command]
#[specta::specta]
pub fn resize_window(app: tauri::AppHandle, label: String, height: f64) { ... }

// Plus the registration entry.
#[::linkme::distributed_slice(crate::specta_builder::KLYNT_COMMANDS)]
#[allow(non_upper_case_globals)]
static __klynt_command_resize_window: crate::specta_builder::CommandRegistration =
    crate::specta_builder::CommandRegistration {
        name: "resize_window",
        invoke: __cmd__resize_window,
        source: crate::specta_builder::SourceKind::Raw,
    };
```

### Failure modes

| Misuse | Compile error |
|---|---|
| Applied to a non-`fn` item (`struct`, `mod`, `impl`) | `klynt_raw_command can only be applied to functions` |

That's the entire list. If the developer forgets `#[tauri::command]`, Tauri's own type-checking catches it at the registration line — span lands there, error is clear.

### Boundary rule (mechanical)

> **Use `#[klynt_command]` if and only if the function:**
> - is `pub async fn`,
> - takes no `state` parameter,
> - returns a bare type `T` (not `Result<T, _>`, not `CommandResult<T>`),
> - and uses `State<'_, Arc<AppCore>>`.
>
> **Otherwise, use `#[klynt_raw_command]`.**

A developer or agent following this rule picks correctly every time. Either macro will refuse mismatched input (raw is loosest; happy-path is strictest).

### Inventory split

Approximate breakdown across the 465-command surface:

| Class | Macro | Count |
|---|---|---|
| Default shape (AppCore + async + Result wrap) | `#[klynt_command]` | ~430 |
| `app: AppHandle` commands (mutating) | `#[klynt_command]` (extra param, no special syntax) | included in 430 |
| FocusTimer or other state | `#[klynt_raw_command]` | ~5 |
| No state (window, permissions) | `#[klynt_raw_command]` | ~6 |
| Sync commands | `#[klynt_raw_command]` | ~3 |
| `rename_all = "camelCase"` (5 finance) | `#[klynt_raw_command]` | 5 |
| `rename_all = "snake_case"` (21 productivity) | `#[klynt_raw_command]` | 21 |
| **Total** | | **~93/7 split** |

---

## Registration & runtime dispatch

### `tauri_specta::Builder` is macro-only

Critical constraint: `tauri_specta::Commands<R>` has crate-private fields and contains a non-closure `fn` pointer. There is no public constructor. Runtime command lists CANNOT be fed to `tauri_specta::Builder::commands(...)` — it must receive the output of the `collect_commands![...]` macro at compile time.

This means the linkme slice cannot be the source of truth for specta TS binding generation. The two registration concerns split:

- **Tauri runtime invoke dispatch** uses `linkme` (Tauri's `invoke_handler` accepts any `Box<dyn Fn>`).
- **Specta TS binding generation** keeps the manual `collect_commands![path1, path2, ...]` list.
- A drift test asserts the two sets match.

### Tauri invoke handler

```rust
// crates/desktop/src/specta_builder.rs
pub fn klynt_invoke_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    use std::collections::HashMap;
    let table: HashMap<&'static str, fn(tauri::ipc::Invoke<tauri::Wry>) -> bool> =
        KLYNT_COMMANDS.iter().map(|c| (c.name, c.invoke)).collect();

    move |invoke| {
        match table.get(invoke.message.command()) {
            Some(f) => f(invoke),
            None => false,
        }
    }
}
```

Wired in `main.rs`:

```rust
.invoke_handler(klynt_invoke_handler())
```

(Replaces Plan 5's `.invoke_handler(specta.invoke_handler())`. No functional regression — specta's wrapper only adds tracking that's unused in the codebase's `ErrorHandlingMode::Throw` configuration.)

### Specta TS binding generation — unchanged

```rust
// crates/desktop/src/specta_builder.rs (unchanged from Plan 5)
pub fn build_specta() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            crate::commands::tasks::task_get,
            crate::commands::tasks::task_list,
            // ... 465 hand-listed paths
        ])
        .events(tauri_specta::collect_events![
            // ... event payload types
        ])
}
```

### Drift test

```rust
// crates/desktop/tests/registration_drift.rs
use std::collections::BTreeSet;
use desktop::specta_builder::{KLYNT_COMMANDS, build_specta};

#[test]
fn linkme_and_specta_lists_match() {
    let linkme: BTreeSet<&str> = KLYNT_COMMANDS.iter().map(|c| c.name).collect();

    let specta_handler = build_specta();
    let specta: BTreeSet<&str> = specta_handler
        .commands_iter()
        .map(|c| c.name)
        .collect();

    let missing_in_specta: Vec<&&str> = linkme.difference(&specta).collect();
    let extra_in_specta: Vec<&&str> = specta.difference(&linkme).collect();

    assert!(
        missing_in_specta.is_empty() && extra_in_specta.is_empty(),
        "Registration drift!\n  In linkme but not specta (add to collect_commands!): {missing_in_specta:?}\n  In specta but not linkme (remove from collect_commands!): {extra_in_specta:?}"
    );
}
```

Runs in <50ms. Fails fast with an actionable message.

### What gets deleted

| Mechanism (pre-Plan-6) | Replacement |
|---|---|
| `pub(crate) const DEV_COMMANDS: &[&str] = &[...]` in 50 modules | Iterate `KLYNT_COMMANDS` slice |
| `tauri_command_names()` source-grep in `dev_server/mod.rs:195-205` | One-line iter over the slice |
| `dev_command_names()` (aggregator over per-module DEV_COMMANDS) | Deleted |
| `dev_server_covers_all_tauri_commands` test | Deleted (tautological) |
| `dev_server_has_no_orphan_commands` test | Deleted (tautological) |
| `tauri::generate_handler![...]` 465-line invocation | `klynt_invoke_handler()` |

Net code-size delta: **~−1700 to −2500 LOC**.

---

## Migration strategy

The defining constraint: every commit must produce a green build and working binary. Five phases, each independently revertable.

### Phase A — Infrastructure (no command changes)

1. Create `crates/desktop-macros/` with stub macros that emit input unchanged.
2. Add `linkme` to `crates/desktop/Cargo.toml`.
3. Add `KLYNT_COMMANDS` slice + `CommandRegistration` struct + `SourceKind` enum to `specta_builder.rs`.
4. Add `klynt_invoke_handler()` — at this point dispatches to a slice with zero entries.
5. Wire the **dual-dispatch** invoke handler in `main.rs`:

```rust
let klynt = klynt_invoke_handler();
let legacy = tauri::generate_handler![/* current 465 commands */];

.invoke_handler(move |invoke| {
    let name = invoke.message.command();
    if KLYNT_COMMANDS.iter().any(|c| c.name == name) {
        klynt(invoke)
    } else {
        legacy(invoke)
    }
})
```

Both paths are live; the slice is the source of truth for "is this command migrated yet."

**Verify:** every existing command works (all routed through `legacy`).

### Phase B — Pilot module

Pick the smallest commands module (`commands/permissions.rs`, 2 commands; or `commands/status.rs`, 1 command). Migrate it.

For each migrated command:

1. Add `#[klynt_command]` (or `#[klynt_raw_command]` if non-default-shape).
2. **Remove from the `legacy` `generate_handler![...]` list** in `main.rs`.
3. **Remove from `pub(crate) const DEV_COMMANDS`** in the module.
4. **Add the migrated command's path to `collect_commands![...]`** in `specta_builder.rs`.

The dual-dispatcher routes migrated commands via `klynt`, others via `legacy`. Both stay green.

### Phase C — Bulk migration (one round per PR, recommended)

| Round | Modules | Commands |
|---|---|---|
| 1 (pilot) | permissions, status | 1–2 each |
| 2 | view, status_badge, knowledge_health, retention_history, review_stats, morning_briefing | 1–3 each |
| 3 | shortcuts, integrations, fabric, entities, entity_links | 2–3 each |
| 4 | objectives, key_results, areas, groups, journey, project_memories, project_sources, project_conversations, pending_memory | 2–4 each |
| 5 | window, focus, language, practice, voice, voice_conversation, distraction, mirror, squads, work_context, workspace, workflows, reforge, settings | 4–12 each |
| 6 | tasks, projects, launcher, timeline, oauth | 1–17 each |
| 7 (heaviest) | finance, productivity, notes | 38, 47, 77 |

After each round: drift test passes; manual UI smoke-test the migrated module.

### Phase D — Switchover

After Phase C round 7, every command goes through one of the macros. Delete the dual-dispatcher; replace with `.invoke_handler(klynt_invoke_handler())`.

### Phase E — Cleanup

1. Delete the two dev_server coverage tests.
2. Replace `tauri_command_names()` body with one-liner that iterates the slice.
3. Delete `dev_command_names()`.
4. Delete any remaining `pub(crate) const DEV_COMMANDS` declarations (Phase C should have caught them all).
5. Update `CLAUDE.md` with the new "Adding a Tauri command" recipe (full text in "Documentation" section below).

### Migration-coexistence test (active during Phases B–D)

```rust
// crates/desktop/tests/no_double_registration.rs
#[test]
fn no_command_double_registered() {
    let legacy: BTreeSet<_> = LEGACY_COMMAND_NAMES.iter().copied().collect();
    let klynt: BTreeSet<_> = KLYNT_COMMANDS.iter().map(|c| c.name).collect();
    let overlap: Vec<_> = legacy.intersection(&klynt).collect();
    assert!(overlap.is_empty(),
        "Command registered in both legacy and klynt: {overlap:?}");
}
```

`LEGACY_COMMAND_NAMES` is hand-maintained alongside `generate_handler![...]` during migration. The test and the const both go away in Phase E.

### Estimated effort

| Phase | Single-dev sequential | Subagent-parallelized |
|---|---|---|
| A — Infrastructure | 1 day | 1 day |
| B — Pilot | 0.5 day | 0.5 day |
| C — Bulk migration (7 rounds) | 5–10 days | 2–3 days |
| D — Switchover | 1 hour | 1 hour |
| E — Cleanup | 0.5 day | 0.5 day |
| **Total** | **~2 weeks** | **~1 week** |

### Sequencing relative to other plans

- **Plan 5 must land first.** `#[klynt_command]` emits `#[specta::specta]` — meaningful only when specta is in the workspace.
- **Cleanup-quick-wins Phase A (`CommandResult` alias)** is consumed by `#[klynt_command]`. Recommended: bundle `CommandResult` into Plan 6's Phase A — saves a workspace-wide churn over the same 52 files twice.
- **Plans 3 and 4 are unaffected** — they don't touch the command registration layer.

Recommended order: **Plan 5 → Plan 6 (with CommandResult alias rolled in) → remaining cleanup-quick-wins phases**.

---

## Testing strategy

### Macro unit tests via `trybuild`

Located in `crates/desktop-macros/tests/trybuild/`. Each `.rs` file is run through the compiler; pass-cases must compile clean, fail-cases must produce the exact stderr in the matching `.stderr` file.

**Pass cases for `#[klynt_command]`:**

| File | What it tests |
|---|---|
| `pass/minimal.rs` | `#[klynt_command] pub async fn foo() -> i32 { state.foo().await }` |
| `pass/with_arg.rs` | One typed argument |
| `pass/multiple_args.rs` | Three typed arguments |
| `pass/with_app_handle.rs` | `app: tauri::AppHandle` parameter |
| `pass/unit_return.rs` | `-> ()` accepted |
| `pass/complex_return.rs` | `-> Vec<Option<TaskResponse>>` |

**Fail cases for `#[klynt_command]`:**

| File | Expected error fragment |
|---|---|
| `fail/missing_async.rs` | `requires \`pub async fn\`` |
| `fail/missing_pub.rs` | `requires \`pub\`` |
| `fail/declared_state.rs` | `injects \`state\` automatically` |
| `fail/result_return.rs` | `wraps return type for you` |
| `fail/command_result_return.rs` | `wraps return type for you` (for the `CommandResult<T>` case) |
| `fail/applied_to_struct.rs` | `can only be applied to functions` |
| `fail/missing_return_type.rs` | `requires an explicit return type` |

**Pass cases for `#[klynt_raw_command]`:**

| File | What it tests |
|---|---|
| `pass/raw_sync.rs` | Sync function with `app: AppHandle` |
| `pass/raw_camel_case.rs` | `#[tauri::command(rename_all = "camelCase")]` |
| `pass/raw_snake_case.rs` | `#[tauri::command(rename_all = "snake_case")]` |
| `pass/raw_focus_timer_state.rs` | `State<'_, Arc<FocusTimer>>` |
| `pass/raw_permissions.rs` | No state |

**Fail cases for `#[klynt_raw_command]`:**

| File | Expected error |
|---|---|
| `fail/raw_on_struct.rs` | `can only be applied to functions` |

### Drift test

`registration_drift.rs` (specified in the Registration section above).

### Migration-coexistence test

`no_double_registration.rs` (specified in the Migration section above). Lifecycle: Phases B–D; deleted in Phase E.

### Bindings drift test (inherited from Plan 5)

Plan 5's `bindings_are_current` test continues to catch the case where a `#[klynt_command]` is added but `bindings.ts` isn't regenerated. No changes required for Plan 6.

### Anti-bypass test (Plan 6 addition)

```rust
// crates/desktop/tests/no_raw_tauri_command_outside_macros.rs
#[test]
fn no_raw_tauri_command_outside_macros() {
    let output = std::process::Command::new("rg")
        .args(["-l", "#[tauri::command]", "crates/desktop/src/commands/", "crates/desktop/src/oauth/"])
        .output()
        .expect("rg available");
    let files: Vec<_> = String::from_utf8(output.stdout).unwrap().lines().collect();
    for file in &files {
        let content = std::fs::read_to_string(file).unwrap();
        for (i, line) in content.lines().enumerate() {
            if line.contains("#[tauri::command]") {
                let context = content
                    .lines()
                    .skip(i.saturating_sub(3))
                    .take(7)
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    context.contains("klynt_command") || context.contains("klynt_raw_command"),
                    "Raw #[tauri::command] in {file} at line {i} — must be wrapped by #[klynt_command] or #[klynt_raw_command]"
                );
            }
        }
    }
}
```

Lives at `crates/desktop/tests/no_raw_tauri_command_outside_macros.rs`. Runs in <100ms. Hard CI gate; the convention can't degrade.

### Per-command integration tests

No changes. The macro doesn't change runtime semantics; existing tests for each command continue to test behavior unchanged.

### Manual smoke test (per Phase C round)

Each round's PR description includes a checklist of UI affordances for the migrated module. Manual but routine.

---

## Documentation (CLAUDE.md updates in Phase E)

Replace the existing "DEV_COMMANDS gotcha" section with:

```markdown
## Adding a Tauri command (Plan 6)

The IPC surface is gated behind two attribute macros in `crates/desktop-macros/`. Direct `#[tauri::command]` is forbidden in `crates/desktop/src/commands/` and `crates/desktop/src/oauth/` (enforced by `no_raw_tauri_command_outside_macros` test).

### Use `#[klynt_command]` for the happy path

A command qualifies for `#[klynt_command]` if and only if it:
- is `pub async fn`,
- takes no `state` parameter (the macro injects it),
- returns a bare type `T` (not `Result<T, _>`),
- and uses `State<'_, Arc<AppCore>>` (the macro injects).

```rust
#[klynt_command]
pub async fn task_get(id: String) -> Option<TaskResponse> {
    state.task_get(id).await
}
```

### Use `#[klynt_raw_command]` otherwise

For sync commands, non-AppCore state, no-state commands, or `rename_all` overrides:

```rust
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub fn resize_window(app: AppHandle, label: String, height: f64) { ... }
```

The macro accepts whatever signature you give it; it only adds the registration entry.

### Steps after adding a new command

1. **Add the function path to `collect_commands![...]` in `crates/desktop/src/specta_builder.rs`.** The `registration_drift` test will fail until you do this.
2. **Run `cargo tauri dev` once** to regenerate `desktop-ui/src/bindings.ts`. Plan 5's `bindings_are_current` test will fail until you do this.
3. Commit. The two tests above are the only hand-verifiable steps; everything else is automatic.
```

---

## Pre-implementation verifications

Three things must be confirmed before Phase A starts. Each is a one-hour spike, not a research project:

1. **Tauri's `__cmd__<name>` wrapper convention.** Confirm `&__cmd__task_get` is a valid `fn(tauri::ipc::Invoke<tauri::Wry>) -> bool` pointer on the pinned Tauri version. Spike: write a single command, manually take its address, dispatch through it, verify result matches `tauri::generate_handler![task_get]`. If the path is unstable, the macro must use a wrapper closure instead.

2. **`linkme` distributed slice on macOS.** `linkme` 0.3+ is stable on modern macOS, but verify on the project's specific Apple Silicon target. Spike: a single `#[distributed_slice]` push from one crate, iterated from another; assert the entry shows up.

3. **`tauri-specta::Builder::commands_iter()` (or equivalent).** The drift test relies on iterating the registered commands from a built `Builder`. Confirm the API exposes this publicly on `tauri-specta = 2.0.0-rc.21`. If not, the drift test consults a sibling const (`SPECTA_COMMAND_NAMES`) maintained alongside `collect_commands![...]`.

If any verification fails, the design adjusts but doesn't fundamentally change. Item (3) has the cleanest fallback (sibling const). Items (1) and (2) are likely to pass.

---

## Out-of-scope (considered and rejected)

- **Codegen the body from the AppCore method name (Question 1, option C).** Rejected: AppCore renames would produce cryptic macro errors; the "explicit body" convention (option B) gives clean line-numbered errors.
- **Codegen `commands/*.rs` files entirely from `AppCore` annotations (option D).** Rejected: large architectural commitment, hard to reverse, IDE go-to-definition becomes confusing.
- **Pure `linkme` registration including specta TS binding generation (γ).** Impossible — `tauri_specta::Commands<R>` has crate-private fields and a non-closure `fn` pointer; runtime construction is sealed by design.
- **Per-module aggregation macros (γ″).** Same `Commands<R>` constructor problem applies to merging per-module command lists; doesn't solve anything.
- **Build script that generates `collect_commands![...]` from source files (γ‴).** Rejected: build scripts add real maintenance burden; the codebase has so far avoided generated code in `OUT_DIR`. The drift test (γ′) achieves equivalent safety without the cost.
- **Per-attribute options on `#[klynt_command]` (γ alternative for outliers).** Rejected: feature creep risk; the bright line "zero args = canonical IPC shape" is more valuable than the ergonomics of accepting some options.
- **Auto-error-conversion (`KlyntbotError → ApiError`).** Rejected: AppCore handlers already return `Result<T, ApiError>` directly. Verified by recon; no `?`/`From`/`.into()` magic needed.
- **Anti-bypass enforcement via custom clippy lint.** Rejected as overkill; the ripgrep-driven test does the same job in 30 lines.

---

## Decision log

Eight binary decisions made during brainstorming, locked in:

| Decision | Choice | Rationale |
|---|---|---|
| Q1 — ambition | (B) macro injects `state` + wraps `Result`, body explicit | Mutability sweet spot; explicit body keeps cargo errors readable |
| Q2 — registration | (γ′) hybrid: linkme for runtime, hand-list + drift test for specta | tauri-specta sealing forces this; drift test = equivalent safety |
| Q3 — outliers | (C) two macros — happy-path strict, raw-command permissive | Each macro has a focused contract; bright-line boundary |
| α — `Result` return type | hard-refuse | No silent double-wrapping |
| β — declared `state` | hard-refuse | Macro is sole injector |
| γ — macro arguments | zero arguments | Bright line: outliers go through raw |
| δ — boundary rule | mechanical 4-bullet checklist | No judgment calls; agents apply uniformly |
| ε — escape-hatch validation | refuse only non-`fn` items | Trust-by-default for outliers |
| ζ — runtime dispatcher | bypass `specta.invoke_handler()` | No functional loss in `Throw` mode |
| η — table lookup | HashMap | 2 extra lines, O(1) dispatch |
| θ — drift test enforcement | CI-only | `const_fn` over distributed slices isn't a thing |
| ι — migration shape | dual-dispatcher during migration | Every commit stays green |
| κ — phase order | Plan 5 → Plan 6 with CommandResult bundled in | Avoids touching every command file twice |
| λ — Phase C PRs | per-round | Faster review, finer revert |
| μ — `CommandResult<T>` return | hard-refuse | Same anti-double-wrap principle as `Result<T, _>` |
| ν — anti-bypass test | yes, ripgrep-driven | Convention as CI gate, not just docs |
| ξ — `-> ()` return | accepted | Some commands legitimately return nothing |

---

## End of spec

Implementation plan to follow.
