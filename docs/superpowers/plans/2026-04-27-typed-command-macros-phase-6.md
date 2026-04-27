# Typed Command Macros (Plan 6) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 465 hand-annotated `#[tauri::command]` + `#[specta::specta]` command shells with two purpose-built attribute macros — `#[klynt_command]` (strict happy path: `pub async fn` + AppCore state + bare `T` return) and `#[klynt_raw_command]` (permissive escape hatch). Auto-register both into a `linkme::distributed_slice`. After Plan 6: adding a command requires writing the function and listing its path in one place; the linker proves the slice is exhaustive on the runtime side, and a 50ms drift test backstops the specta TS-binding hand-list.

**Architecture:** New crate `crates/desktop-macros/` (proc-macro = true) at workspace L7, sibling of `desktop-shared`. New dep `linkme = "0.3"` in `crates/desktop`. Distributed slice `KLYNT_COMMANDS` declared in `crates/desktop/src/specta_builder.rs`. Each macro emits the user's function (transformed for happy-path; unchanged for raw) PLUS a sibling `__klynt_dispatch_<name>` fn that wraps `tauri::generate_handler![<name>]` PLUS a `#[distributed_slice]` static pushing one `CommandRegistration` entry. New runtime dispatcher `klynt_invoke_handler()` builds a `HashMap<&str, fn>` from the slice and routes `Invoke<R>` accordingly. Migration uses a dual-dispatcher (legacy `generate_handler!` + new `klynt_invoke_handler`) during Phases B–C; Phase D removes legacy.

**Tech Stack:** Rust 1.93, Tauri 2.x (pinned `~2.1` from Plan 5), `tauri-specta = =2.0.0-rc.21` (from Plan 5), `linkme = "0.3"` (new), `proc-macro2`, `syn = "2"`, `quote = "1"`, `trybuild = "1"` for macro UI tests, existing nextest + bun + Vitest test infra.

**Master plan context:** Plan 6 of an extended series. **Hard dependency on Plan 5** (all commands carry `#[specta::specta]`; specta crate setup in place; `bindings.ts` is generated; `bindings_are_current` test exists). **Bundles cleanup-quick-wins Phase A** (the `CommandResult<T>` alias) — saves a workspace-wide churn over the same 52 files. **Independent of Plans 3, 4** (they don't touch the command registration layer).

**Spec:** `docs/superpowers/specs/2026-04-27-typed-command-macros-design.md` (committed `28d3bb307`). Plan tasks reference spec sections by number; read the spec first if unfamiliar.

---

## File Structure

### Files to create

| Path | Responsibility |
|---|---|
| `crates/desktop-macros/Cargo.toml` | Workspace member declaration; `proc-macro = true`; deps on `proc-macro2`, `syn`, `quote`. |
| `crates/desktop-macros/src/lib.rs` | The two `#[proc_macro_attribute]` entry points: `klynt_command` and `klynt_raw_command`. Delegates to per-macro modules. |
| `crates/desktop-macros/src/klynt_command.rs` | Parse → validate → expand for the happy-path attribute. ~150 lines. |
| `crates/desktop-macros/src/klynt_raw_command.rs` | Parse → expand for the escape hatch. ~30 lines. |
| `crates/desktop-macros/src/parse.rs` | Shared `syn` parsing helpers (extract function ident, return type, params). |
| `crates/desktop-macros/src/errors.rs` | `syn::Error::new_spanned` builders for the 7 happy-path error messages + 1 raw error message. |
| `crates/desktop-macros/tests/trybuild.rs` | The `trybuild` driver that walks `tests/trybuild/{pass,fail}/`. |
| `crates/desktop-macros/tests/trybuild/pass/*.rs` (6 files) | Pass cases per spec Section 6. |
| `crates/desktop-macros/tests/trybuild/fail/*.rs` (7 files) | Fail cases per spec Section 6. |
| `crates/desktop-macros/tests/trybuild/fail/*.stderr` (7 files) | Expected error messages, matched exactly. |
| `crates/desktop-macros/tests/trybuild/raw_pass/*.rs` (5 files) | Raw-command pass cases. |
| `crates/desktop-macros/tests/trybuild/raw_fail/*.rs` (1 file) | Raw-command fail case. |
| `crates/desktop/tests/registration_drift.rs` | The 50ms drift test asserting `KLYNT_COMMANDS` and `collect_commands![...]` contain the same names. |
| `crates/desktop/tests/no_double_registration.rs` | Migration-coexistence test (lifecycle: Phases B–D; deleted in E). |
| `crates/desktop/tests/no_raw_tauri_command_outside_macros.rs` | Anti-bypass ripgrep-driven test. |

### Files to modify

| Path | Change |
|---|---|
| `Cargo.toml` (workspace root) | Add `linkme = "0.3"` to `[workspace.dependencies]`; add `desktop-macros` to the `members` list. |
| `crates/desktop-shared/src/errors.rs` | Add `pub type CommandResult<T> = Result<T, ApiError>;` (cleanup-quick-wins Phase A bundled in). |
| `crates/desktop-shared/src/lib.rs` | Re-export `CommandResult`. |
| `crates/desktop/Cargo.toml` | Add `desktop-macros = { path = "../desktop-macros" }` and `linkme = { workspace = true }`. |
| `crates/desktop/src/specta_builder.rs` | Add `KLYNT_COMMANDS` distributed slice declaration; add `CommandRegistration` struct + `SourceKind` enum; add `klynt_invoke_handler()` function. |
| `crates/desktop/src/main.rs:737` | Phase A: wrap existing `generate_handler!` in dual-dispatcher closure that also tries `klynt_invoke_handler()`. Phase D: replace dual-dispatcher with `klynt_invoke_handler()` only. |
| `crates/desktop/src/main.rs` (top of `tauri::Builder` chain) | Phase A: add `LEGACY_COMMAND_NAMES` const used by `no_double_registration` test. Phase E: delete. |
| `crates/desktop/src/commands/*.rs` (52 files including oauth) | Phase C: replace `#[tauri::command]` + `#[specta::specta]` with `#[klynt_command]` (or `#[klynt_raw_command]`). Remove `pub(crate) const DEV_COMMANDS`. |
| `crates/desktop/src/dev_server/mod.rs:195-300` | Phase E: replace `tauri_command_names()` body with one-liner iter over `KLYNT_COMMANDS`. Delete `dev_command_names()` body (no longer needed). Delete `dev_server_covers_all_tauri_commands` and `dev_server_has_no_orphan_commands` tests. |
| `CLAUDE.md` | Phase E: replace "DEV_COMMANDS gotcha" section with "Adding a Tauri command (Plan 6)" recipe. |

### Files NOT modified

- `crates/desktop-shared/src/types.rs`, `errors.rs` (other than `CommandResult` add) — types are stable.
- `crates/desktop/src/specta_builder.rs::build_specta()` body — the `collect_commands![...]` 465-line list stays exactly as Plan 5 left it. Drift test enforces parity.
- `desktop-ui/src/bindings.ts` — never edited by Plan 6; specta regenerates it identically.
- `crates/desktop/tests/bindings_are_current.rs` — Plan 5 test continues to backstop the FE-side of the contract.

---

## Phase A — Infrastructure & verification spikes (12 tasks)

### Task A1: Pre-implementation spike — Tauri per-command handler convention

**Goal:** Verify the dispatcher-wrapper approach works on the pinned Tauri version. The spec assumed `__cmd__<name>` was reachable; recon proved it isn't. Confirm the alternative.

**Files:**
- Create: `crates/desktop/tests/spike_tauri_dispatch.rs` (deleted at end of Phase A)

- [ ] **Step 1: Write the spike test**

```rust
//! Spike: confirm we can build a per-command Tauri dispatcher via
//! `tauri::generate_handler![<one_name>]` returning `Box<dyn Fn(Invoke<R>) -> bool + Send + Sync>`.

#[tauri::command]
async fn spike_cmd(x: i32) -> Result<i32, String> {
    Ok(x * 2)
}

#[test]
fn per_command_handler_constructible() {
    // The handler must be storable as a fn pointer or boxed closure that
    // takes Invoke<Wry> and returns bool. If this compiles, our macro
    // dispatcher strategy works.
    fn _check_handler_type() {
        let _h: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> =
            Box::new(tauri::generate_handler![spike_cmd]);
    }
    _check_handler_type();
}
```

- [ ] **Step 2: Run the spike**

Run: `cargo nextest run -p desktop --test spike_tauri_dispatch 2>&1 | tail -20`
Expected: pass.

- [ ] **Step 3: If it fails — STOP and reconsider**

If `tauri::generate_handler![spike_cmd]` doesn't yield a boxable closure, Plan 6's dispatch strategy is broken. Open an issue and pause the plan. Most likely cause: API change in a Tauri 2.x point release. Workaround: use `tauri::Builder::invoke_handler` directly with `linkme`-driven name match in the closure body, calling each command's handler via macro-expanded shim.

- [ ] **Step 4: Delete the spike test**

```bash
rm crates/desktop/tests/spike_tauri_dispatch.rs
```

- [ ] **Step 5: No commit needed — verification only.**

---

### Task A2: Pre-implementation spike — `linkme` distributed slice on macOS

**Goal:** Verify `linkme = "0.3"` works on the project's Apple Silicon target (Plan 6 depends on it).

**Files:**
- Create: `crates/desktop/tests/spike_linkme.rs` (deleted at end of Phase A)
- Modify: `crates/desktop/Cargo.toml` (temporarily add `linkme` dep)

- [ ] **Step 1: Add `linkme` to dev-dependencies**

```toml
[dev-dependencies]
linkme = "0.3"
```

- [ ] **Step 2: Write the spike test**

```rust
use linkme::distributed_slice;

#[distributed_slice]
static SPIKE_ENTRIES: [&'static str] = [..];

#[distributed_slice(SPIKE_ENTRIES)]
static FIRST: &str = "hello";

#[distributed_slice(SPIKE_ENTRIES)]
static SECOND: &str = "world";

#[test]
fn linkme_aggregates_entries() {
    let entries: Vec<&str> = SPIKE_ENTRIES.iter().copied().collect();
    assert!(entries.contains(&"hello"));
    assert!(entries.contains(&"world"));
    assert_eq!(entries.len(), 2);
}
```

- [ ] **Step 3: Run the spike**

Run: `cargo nextest run -p desktop --test spike_linkme 2>&1 | tail -10`
Expected: pass.

- [ ] **Step 4: If it fails — STOP**

Most likely failure: macOS `mod_init_func` ordering quirk. Modern macOS + linkme 0.3 should be clean. If it fails, file an issue and pause.

- [ ] **Step 5: Delete the spike + revert Cargo.toml**

```bash
rm crates/desktop/tests/spike_linkme.rs
```

Revert the dev-dependencies addition (it'll be re-added permanently in A4).

- [ ] **Step 6: No commit — verification only.**

---

### Task A3: Add `CommandResult<T>` type alias (cleanup-quick-wins Phase A bundled in)

**Files:**
- Modify: `crates/desktop-shared/src/errors.rs`
- Modify: `crates/desktop-shared/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to the bottom of `crates/desktop-shared/src/errors.rs`:

```rust
#[cfg(test)]
mod alias_tests {
    use super::*;

    #[test]
    fn command_result_is_alias_for_result_apierror() {
        let v: CommandResult<i32> = Ok(42);
        assert_eq!(v.unwrap(), 42);
        let e: CommandResult<()> = Err(ApiError::new("TestKind", "test"));
        assert!(e.is_err());
    }
}
```

- [ ] **Step 2: Run — expected to fail**

Run: `cargo nextest run -p desktop-shared command_result_is_alias_for_result_apierror`
Expected: FAIL with "cannot find type `CommandResult`".

- [ ] **Step 3: Add the alias**

Add to `errors.rs` after the `ApiError` impl block:

```rust
/// Convenience alias used by every `#[tauri::command]` in `crates/desktop`.
pub type CommandResult<T> = Result<T, ApiError>;
```

- [ ] **Step 4: Re-export from `lib.rs`**

Find the existing `pub use errors::ApiError;` (or equivalent) and extend:

```rust
pub use errors::{ApiError, CommandResult};
```

- [ ] **Step 5: Run the test — expected to pass**

Run: `cargo nextest run -p desktop-shared command_result_is_alias_for_result_apierror`
Expected: pass.

- [ ] **Step 6: Build the workspace**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop-shared/src/errors.rs crates/desktop-shared/src/lib.rs
git commit -m "feat(desktop-shared): add CommandResult<T> type alias"
```

---

### Task A4: Add `linkme` to workspace deps + desktop crate

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/desktop/Cargo.toml`

- [ ] **Step 1: Add to workspace deps**

In root `Cargo.toml`'s `[workspace.dependencies]` block (alphabetical):

```toml
linkme = "0.3"
```

- [ ] **Step 2: Add to `crates/desktop/Cargo.toml`**

In `[dependencies]`:

```toml
linkme = { workspace = true }
```

- [ ] **Step 3: Build to confirm resolution**

Run: `cargo build -p desktop 2>&1 | tail -10`
Expected: clean. (linkme not yet referenced from any code, so it just resolves into Cargo.lock.)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/desktop/Cargo.toml Cargo.lock
git commit -m "chore(workspace): add linkme dep for Plan 6 distributed slice"
```

---

### Task A5: Create `desktop-macros` crate skeleton

**Files:**
- Create: `crates/desktop-macros/Cargo.toml`
- Create: `crates/desktop-macros/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add to `members`

- [ ] **Step 1: Create `crates/desktop-macros/Cargo.toml`**

```toml
[package]
name = "desktop-macros"
version = "0.1.0"
edition = "2024"

[lib]
proc-macro = true

[dependencies]
proc-macro2 = "1"
syn = { version = "2", features = ["full"] }
quote = "1"

[dev-dependencies]
trybuild = "1"
```

- [ ] **Step 2: Create `crates/desktop-macros/src/lib.rs`**

```rust
//! Klynt's Tauri command attribute macros. See
//! `docs/superpowers/specs/2026-04-27-typed-command-macros-design.md`.

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn klynt_command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Phase A1 stub: emit input unchanged so existing commands keep working
    // until Phase B writes the real expansion.
    item
}

#[proc_macro_attribute]
pub fn klynt_raw_command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
```

- [ ] **Step 3: Add to workspace members**

Edit root `Cargo.toml`'s `[workspace] members` list — add `"crates/desktop-macros"`.

- [ ] **Step 4: Build the new crate**

Run: `cargo build -p desktop-macros 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/desktop-macros/
git commit -m "feat(desktop-macros): add proc-macro crate skeleton (stub macros)"
```

---

### Task A6: Wire `desktop-macros` into the `desktop` crate

**Files:**
- Modify: `crates/desktop/Cargo.toml`

- [ ] **Step 1: Add the dep**

In `[dependencies]`:

```toml
desktop-macros = { path = "../desktop-macros" }
```

- [ ] **Step 2: Build**

Run: `cargo build -p desktop 2>&1 | tail -5`
Expected: clean. The macros aren't used yet.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/Cargo.toml
git commit -m "chore(desktop): depend on desktop-macros"
```

---

### Task A7: Add `KLYNT_COMMANDS` slice + `CommandRegistration` to `specta_builder.rs`

**Files:**
- Modify: `crates/desktop/src/specta_builder.rs`

- [ ] **Step 1: Read the existing file head**

Run: `head -25 crates/desktop/src/specta_builder.rs`
Expected: Plan 5's imports + `build_specta()` signature.

- [ ] **Step 2: Add the slice + struct + enum**

Insert after the existing imports, before `build_specta()`:

```rust
use linkme::distributed_slice;
use std::collections::HashMap;

/// One element per command, populated at link time by the `#[klynt_command]`
/// and `#[klynt_raw_command]` macros. The slice IS the truth for which commands
/// are runtime-dispatchable.
#[distributed_slice]
pub static KLYNT_COMMANDS: [CommandRegistration] = [..];

#[derive(Copy, Clone)]
pub struct CommandRegistration {
    pub name: &'static str,
    pub invoke: fn(::tauri::ipc::Invoke<::tauri::Wry>) -> bool,
    pub source: SourceKind,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SourceKind {
    Klynt,
    Raw,
}

/// Runtime invoke-handler that dispatches via the linkme-collected slice.
/// Replaces `tauri::generate_handler![...]` once Phase D switchover lands.
pub fn klynt_invoke_handler() -> impl Fn(::tauri::ipc::Invoke<::tauri::Wry>) -> bool + Send + Sync + 'static {
    let table: HashMap<&'static str, fn(::tauri::ipc::Invoke<::tauri::Wry>) -> bool> =
        KLYNT_COMMANDS.iter().map(|c| (c.name, c.invoke)).collect();

    move |invoke| {
        match table.get(invoke.message.command()) {
            Some(f) => f(invoke),
            None => false,
        }
    }
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p desktop 2>&1 | tail -10`
Expected: clean. Slice is empty; `klynt_invoke_handler()` builds an empty table.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/specta_builder.rs
git commit -m "feat(desktop): add KLYNT_COMMANDS slice + klynt_invoke_handler skeleton"
```

---

### Task A8: Wire dual-dispatcher in `main.rs`

**Files:**
- Modify: `crates/desktop/src/main.rs`

The legacy `tauri::generate_handler![...]` at line 737 stays — we wrap it. Migrated commands route through `klynt_invoke_handler`; un-migrated through `legacy`.

- [ ] **Step 1: Read the current invoke_handler line**

Run: `grep -n "invoke_handler\|generate_handler!" crates/desktop/src/main.rs | head -5`
Expected: at least one match around line 737.

- [ ] **Step 2: Wrap the existing handler in a dual-dispatcher**

Find the `.invoke_handler(tauri::generate_handler![...])` line. Replace with:

```rust
.invoke_handler({
    let klynt = crate::specta_builder::klynt_invoke_handler();
    let legacy = tauri::generate_handler![
        // existing 465-command list — unchanged from before this task
    ];

    move |invoke| {
        let name = invoke.message.command();
        if crate::specta_builder::KLYNT_COMMANDS.iter().any(|c| c.name == name) {
            klynt(invoke)
        } else {
            legacy(invoke)
        }
    }
})
```

(The `[...existing 465 list...]` part is left exactly as it was — no changes to that giant list yet.)

- [ ] **Step 3: Build**

Run: `cargo build -p desktop 2>&1 | tail -15`
Expected: clean.

- [ ] **Step 4: Smoke-test the binary**

Run: `cargo tauri dev` (in another terminal)
Open the desktop app. Click around — every command still works because the slice is empty so every `invoke` falls through to `legacy`.
Expected: identical behaviour to pre-Phase-A.

Stop the dev process when satisfied.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(desktop): wire dual-dispatcher (legacy + klynt) — slice still empty"
```

---

### Task A9: Add the `LEGACY_COMMAND_NAMES` const + coexistence test

**Files:**
- Modify: `crates/desktop/src/main.rs`
- Create: `crates/desktop/tests/no_double_registration.rs`

The `no_double_registration` test asserts no command appears in both `LEGACY_COMMAND_NAMES` and `KLYNT_COMMANDS`. Lifecycle: Phases A–D; deleted in E.

- [ ] **Step 1: Add `LEGACY_COMMAND_NAMES` const in `main.rs`**

Above the `tauri::Builder::default()` chain:

```rust
/// Snapshot of every command currently registered via `tauri::generate_handler!`.
/// Hand-maintained alongside the `generate_handler!` list during the Plan 6
/// migration. **Deleted in Phase E.** The `no_double_registration` test
/// asserts no command name appears in both this list and `KLYNT_COMMANDS`.
pub const LEGACY_COMMAND_NAMES: &[&str] = &[
    "task_get",
    "task_list",
    "task_create",
    // ... all 465 names — paste from the existing generate_handler! list
];
```

(To populate the list mechanically: `grep -oE "commands::[a-z_]+::[a-z_]+" crates/desktop/src/main.rs | sed -E 's/.*::([a-z_]+)/"\1",/' > /tmp/cmd_names.txt`, then paste into the const body.)

- [ ] **Step 2: Create the test**

`crates/desktop/tests/no_double_registration.rs`:

```rust
//! Coexistence guard. Runs during Phase B–D (commands in legacy AND klynt is illegal).
//! Deleted in Phase E along with `LEGACY_COMMAND_NAMES`.

use std::collections::BTreeSet;
use desktop::main::LEGACY_COMMAND_NAMES;
use desktop::specta_builder::KLYNT_COMMANDS;

#[test]
fn no_command_double_registered() {
    let legacy: BTreeSet<&str> = LEGACY_COMMAND_NAMES.iter().copied().collect();
    let klynt: BTreeSet<&str> = KLYNT_COMMANDS.iter().map(|c| c.name).collect();
    let overlap: Vec<&&str> = legacy.intersection(&klynt).collect();
    assert!(
        overlap.is_empty(),
        "Command in both legacy and klynt slices: {overlap:?}\n\
         Phase C migration drops names from `LEGACY_COMMAND_NAMES` as it adds them via `#[klynt_command]`.\n\
         If a name appears in both, the corresponding Phase C task forgot to remove it from main.rs."
    );
}
```

(Note: `desktop::main::LEGACY_COMMAND_NAMES` requires the `LEGACY_COMMAND_NAMES` const to be reachable as a public item from `desktop::main`. If `main.rs` is binary-only, declare the const in `crates/desktop/src/lib.rs` instead and import in `main.rs`. Plan 5 added `lib.rs` for `specta_builder` — extend it.)

- [ ] **Step 3: Run the test**

Run: `cargo nextest run -p desktop no_command_double_registered`
Expected: pass (slice is empty, so intersection is empty).

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/main.rs crates/desktop/src/lib.rs crates/desktop/tests/no_double_registration.rs
git commit -m "feat(desktop): add LEGACY_COMMAND_NAMES + no_double_registration test"
```

---

### Task A10: Implement `#[klynt_command]` macro body

**Files:**
- Modify: `crates/desktop-macros/src/lib.rs`
- Create: `crates/desktop-macros/src/klynt_command.rs`
- Create: `crates/desktop-macros/src/parse.rs`
- Create: `crates/desktop-macros/src/errors.rs`

This is the most consequential single task in the plan. The macro:

1. Parses the input as `syn::ItemFn`.
2. Validates: `pub`, `async`, has return type, return type isn't `Result<T, _>` or `CommandResult<T>`, no `state` parameter.
3. Injects `state: ::tauri::State<'_, ::std::sync::Arc<crate::app_core::AppCore>>` as first param.
4. Wraps return type `T` to `::desktop_shared::CommandResult<T>`.
5. Emits `#[tauri::command]` and `#[specta::specta]`.
6. Emits a sibling `__klynt_dispatch_<name>` fn that wraps `tauri::generate_handler![<name>]`.
7. Pushes a `CommandRegistration` entry into `KLYNT_COMMANDS`.

- [ ] **Step 1: Write the parse helper**

`crates/desktop-macros/src/parse.rs`:

```rust
use syn::{FnArg, ItemFn, Pat, PatIdent, ReturnType, Type};

pub struct ParsedCommand {
    pub fn_item: ItemFn,
}

impl ParsedCommand {
    pub fn declared_state_param(&self) -> Option<&FnArg> {
        self.fn_item.sig.inputs.iter().find(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                if let Pat::Ident(PatIdent { ident, .. }) = &*pat_type.pat {
                    return ident == "state";
                }
            }
            false
        })
    }

    pub fn return_type(&self) -> Option<&Type> {
        if let ReturnType::Type(_, ty) = &self.fn_item.sig.output {
            Some(ty)
        } else {
            None
        }
    }

    pub fn return_type_is_result(&self) -> bool {
        // Detects `Result<T, _>` or `CommandResult<T>` literally.
        let Some(ty) = self.return_type() else { return false; };
        let s = quote::quote!(#ty).to_string();
        s.starts_with("Result <") || s.starts_with("CommandResult <")
            || s.contains(":: CommandResult <") || s.contains(":: Result <")
    }
}
```

- [ ] **Step 2: Write the error helper**

`crates/desktop-macros/src/errors.rs`:

```rust
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::spanned::Spanned;

pub fn err<T: ToTokens>(spanned: T, message: &str) -> TokenStream {
    syn::Error::new_spanned(spanned, message).to_compile_error()
}

pub const ERR_MISSING_PUB: &str = "klynt_command requires `pub`";
pub const ERR_MISSING_ASYNC: &str = "klynt_command requires `pub async fn`";
pub const ERR_DECLARED_STATE: &str =
    "klynt_command injects `state` automatically — remove this parameter";
pub const ERR_RESULT_RETURN: &str =
    "klynt_command wraps return type for you — declare bare `T` instead of `Result<T, ApiError>` or `CommandResult<T>`";
pub const ERR_MISSING_RETURN: &str = "klynt_command requires an explicit return type";
pub const ERR_NOT_FUNCTION: &str = "klynt_command can only be applied to functions";
```

- [ ] **Step 3: Write the happy-path expansion**

`crates/desktop-macros/src/klynt_command.rs`:

```rust
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, ItemFn, ReturnType, Type, Visibility};

use crate::{errors::*, parse::ParsedCommand};

pub fn expand(input: TokenStream) -> TokenStream {
    let fn_item: ItemFn = match parse2(input.clone()) {
        Ok(f) => f,
        Err(_) => return err(input, ERR_NOT_FUNCTION),
    };

    let parsed = ParsedCommand { fn_item };

    if !matches!(parsed.fn_item.vis, Visibility::Public(_)) {
        return err(&parsed.fn_item.sig.fn_token, ERR_MISSING_PUB);
    }
    if parsed.fn_item.sig.asyncness.is_none() {
        return err(&parsed.fn_item.sig.fn_token, ERR_MISSING_ASYNC);
    }
    if let Some(state_param) = parsed.declared_state_param() {
        return err(state_param, ERR_DECLARED_STATE);
    }
    if parsed.return_type().is_none() {
        return err(&parsed.fn_item.sig, ERR_MISSING_RETURN);
    }
    if parsed.return_type_is_result() {
        let ty = parsed.return_type().unwrap();
        return err(ty, ERR_RESULT_RETURN);
    }

    let return_ty = parsed.return_type().unwrap();
    let fn_ident = &parsed.fn_item.sig.ident;
    let fn_vis = &parsed.fn_item.vis;
    let fn_async = &parsed.fn_item.sig.asyncness;
    let fn_inputs = &parsed.fn_item.sig.inputs;
    let fn_block = &parsed.fn_item.block;
    let fn_attrs = &parsed.fn_item.attrs;
    let dispatcher_ident = format_ident!("__klynt_dispatch_{}", fn_ident);
    let registration_ident = format_ident!("__klynt_command_{}", fn_ident);
    let fn_name_str = fn_ident.to_string();

    quote! {
        #[::tauri::command]
        #[::specta::specta]
        #(#fn_attrs)*
        #fn_vis #fn_async fn #fn_ident(
            state: ::tauri::State<'_, ::std::sync::Arc<crate::app_core::AppCore>>,
            #fn_inputs
        ) -> ::desktop_shared::CommandResult<#return_ty> {
            #fn_block
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #dispatcher_ident(invoke: ::tauri::ipc::Invoke<::tauri::Wry>) -> bool {
            (::tauri::generate_handler![#fn_ident])(invoke)
        }

        #[::linkme::distributed_slice(crate::specta_builder::KLYNT_COMMANDS)]
        #[allow(non_upper_case_globals)]
        static #registration_ident: crate::specta_builder::CommandRegistration =
            crate::specta_builder::CommandRegistration {
                name: #fn_name_str,
                invoke: #dispatcher_ident,
                source: crate::specta_builder::SourceKind::Klynt,
            };
    }
}
```

- [ ] **Step 4: Wire into `lib.rs`**

`crates/desktop-macros/src/lib.rs`:

```rust
use proc_macro::TokenStream;

mod errors;
mod klynt_command;
mod klynt_raw_command;
mod parse;

#[proc_macro_attribute]
pub fn klynt_command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    klynt_command::expand(item.into()).into()
}

#[proc_macro_attribute]
pub fn klynt_raw_command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    klynt_raw_command::expand(item.into()).into()
}
```

- [ ] **Step 5: Stub `klynt_raw_command.rs` for now**

`crates/desktop-macros/src/klynt_raw_command.rs`:

```rust
use proc_macro2::TokenStream;

pub fn expand(input: TokenStream) -> TokenStream {
    // Implemented in A11.
    input
}
```

- [ ] **Step 6: Build**

Run: `cargo build -p desktop-macros 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop-macros/src/
git commit -m "feat(desktop-macros): implement #[klynt_command] happy-path expansion"
```

---

### Task A11: Implement `#[klynt_raw_command]` macro body

**Files:**
- Modify: `crates/desktop-macros/src/klynt_raw_command.rs`

- [ ] **Step 1: Write the expansion**

```rust
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, ItemFn};

use crate::errors::{err, ERR_NOT_FUNCTION};

pub fn expand(input: TokenStream) -> TokenStream {
    let fn_item: ItemFn = match parse2(input.clone()) {
        Ok(f) => f,
        Err(_) => return err(input, ERR_NOT_FUNCTION),
    };

    let fn_ident = &fn_item.sig.ident;
    let dispatcher_ident = format_ident!("__klynt_dispatch_{}", fn_ident);
    let registration_ident = format_ident!("__klynt_command_{}", fn_ident);
    let fn_name_str = fn_ident.to_string();

    quote! {
        #fn_item

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #dispatcher_ident(invoke: ::tauri::ipc::Invoke<::tauri::Wry>) -> bool {
            (::tauri::generate_handler![#fn_ident])(invoke)
        }

        #[::linkme::distributed_slice(crate::specta_builder::KLYNT_COMMANDS)]
        #[allow(non_upper_case_globals)]
        static #registration_ident: crate::specta_builder::CommandRegistration =
            crate::specta_builder::CommandRegistration {
                name: #fn_name_str,
                invoke: #dispatcher_ident,
                source: crate::specta_builder::SourceKind::Raw,
            };
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p desktop-macros 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-macros/src/klynt_raw_command.rs
git commit -m "feat(desktop-macros): implement #[klynt_raw_command] passthrough+register"
```

---

### Task A12: trybuild test infrastructure + first pass case

**Files:**
- Create: `crates/desktop-macros/tests/trybuild.rs`
- Create: `crates/desktop-macros/tests/trybuild/pass/minimal.rs`

- [ ] **Step 1: Create the trybuild driver**

`crates/desktop-macros/tests/trybuild.rs`:

```rust
//! Macro UI tests via the `trybuild` crate. Each `.rs` in `pass/` must
//! compile clean; each `.rs` in `fail/` must produce the exact stderr
//! in the matching `.stderr` file.

#[test]
fn ui_pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/pass/*.rs");
}

#[test]
fn ui_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/fail/*.rs");
}
```

- [ ] **Step 2: Create the smallest pass case**

`crates/desktop-macros/tests/trybuild/pass/minimal.rs`:

```rust
//! Minimal `#[klynt_command]` smoke test — must compile.

// Trybuild tests run against a stub linkme slice + AppCore type.
mod crate_stubs {
    pub mod app_core { pub struct AppCore; }
    pub mod specta_builder {
        use super::app_core::AppCore;
        pub struct CommandRegistration {
            pub name: &'static str,
            pub invoke: fn(::tauri::ipc::Invoke<::tauri::Wry>) -> bool,
            pub source: SourceKind,
        }
        pub enum SourceKind { Klynt, Raw }
        #[::linkme::distributed_slice]
        pub static KLYNT_COMMANDS: [CommandRegistration] = [..];
    }
}
use crate_stubs as crate_;

#[desktop_macros::klynt_command]
pub async fn ping() -> i32 {
    42
}

fn main() {}
```

(Note: trybuild tests live in their own crate. The stubs let the macro expansion compile without depending on the real `desktop` crate.)

- [ ] **Step 3: Run the trybuild driver**

Run: `cargo nextest run -p desktop-macros ui_pass 2>&1 | tail -20`
Expected: pass.

- [ ] **Step 4: If trybuild infrastructure isn't ready** — the path-resolution stubs may need tweaking. Iterate until `pass/minimal.rs` compiles inside trybuild.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-macros/tests/trybuild.rs crates/desktop-macros/tests/trybuild/pass/minimal.rs
git commit -m "test(desktop-macros): add trybuild driver + first pass case"
```

---

## Phase B — Pilot module migration (1 task)

### Task B1: Pilot — migrate `commands/status.rs` (1 command, happy-path)

**Files:**
- Modify: `crates/desktop/src/commands/status.rs`
- Modify: `crates/desktop/src/main.rs` — drop `agent_status` from `LEGACY_COMMAND_NAMES` and `generate_handler![...]`
- Modify: `crates/desktop/src/specta_builder.rs::build_specta()` — add `crate::commands::status::agent_status` to `collect_commands![...]` (already there from Plan 5; verify)

The pilot validates the entire Phase A toolchain: macro expansion, slice push, dispatcher wrap, dual-dispatcher routing.

- [ ] **Step 1: Read `status.rs`**

Run: `cat crates/desktop/src/commands/status.rs`
Expected: one `#[tauri::command] #[specta::specta]` annotated function returning `Result<AgentStatusResponse, ApiError>`.

- [ ] **Step 2: Replace the two attributes with `#[klynt_command]`**

Before:

```rust
use desktop_shared::commands::AgentStatusResponse;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;
use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn agent_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<AgentStatusResponse, ApiError> {
    state.agent_status().await
}

pub(crate) const DEV_COMMANDS: &[&str] = &["agent_status"];
```

After:

```rust
use desktop_shared::commands::AgentStatusResponse;
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn agent_status() -> AgentStatusResponse {
    state.agent_status().await
}

// DEV_COMMANDS removed — KLYNT_COMMANDS slice is the truth now.
```

(The `state` binding in the body comes from the macro-injected first parameter.)

- [ ] **Step 3: Build**

Run: `cargo build -p desktop 2>&1 | tail -15`
Expected: clean. The macro emits all the necessary attributes plus the linkme push.

- [ ] **Step 4: Remove `agent_status` from `LEGACY_COMMAND_NAMES`**

Find the line `"agent_status",` in `LEGACY_COMMAND_NAMES` (in `main.rs` or `lib.rs`) and delete it.

- [ ] **Step 5: Remove `agent_status` from the legacy `tauri::generate_handler![...]`**

Find `commands::status::agent_status` in the `generate_handler![...]` list and delete the line.

- [ ] **Step 6: Build + run all relevant tests**

```bash
cargo build -p desktop
cargo nextest run -p desktop
```

Expected:
- Build clean.
- `no_double_registration` test passes (agent_status now only in klynt slice).
- `bindings_are_current` test passes (TS bindings unchanged — specta still has the same surface).

- [ ] **Step 7: Smoke-test the binary**

Run: `cargo tauri dev`. In the desktop app, navigate to wherever `agent_status` is called (likely the status bar). Confirm it works.

- [ ] **Step 8: Commit**

```bash
git add crates/desktop/src/commands/status.rs crates/desktop/src/main.rs crates/desktop/src/lib.rs
git commit -m "feat(desktop): pilot — migrate commands/status to #[klynt_command]"
```

---

## Phase C — Bulk migration (51 module tasks across 7 rounds)

Each module task follows the **B1 template**: replace attributes, drop from `LEGACY_COMMAND_NAMES`, drop from `generate_handler![...]`, build, test, commit.

**Common steps for every Phase C task:**

1. Read the module file.
2. For each `#[tauri::command] #[specta::specta]` annotated function:
   - If it fits the happy-path (`pub async fn`, AppCore state, `Result<T, ApiError>` return) → replace both attributes with `#[klynt_command]`. Drop the `state` parameter (macro injects). Drop the `Result<_, ApiError>` wrapping (macro wraps).
   - Otherwise → replace `#[tauri::command]` with `#[klynt_raw_command]` first, then keep `#[tauri::command]` (or its variant) and `#[specta::specta]` after it. Function signature unchanged.
3. Delete `pub(crate) const DEV_COMMANDS: &[&str] = &[...]` from the module.
4. For every command in this module, delete its name from `LEGACY_COMMAND_NAMES` in `crates/desktop/src/lib.rs` (or `main.rs`).
5. For every command in this module, delete its `commands::<module>::<name>` line from the legacy `generate_handler![...]` list in `main.rs`.
6. Build: `cargo build -p desktop`.
7. Run: `cargo nextest run -p desktop`.
8. Smoke-test the migrated module's UI surface in `cargo tauri dev`.
9. Commit: `feat(desktop): migrate commands/<module> to klynt macros`.

### Round 1 — 1-command pilots (already done in B1; remaining 6 tasks)

All happy-path. Apply common steps above.

| Task | Module | Commands |
|---|---|---|
| C1.1 | `commands/morning_briefing.rs` | `morning_briefing_summary` |
| C1.2 | `commands/status_badge.rs` | `show_status_badge` |
| C1.3 | `commands/timeline.rs` | `timeline_query` |
| C1.4 | `commands/review_stats.rs` | `review_stats_summary` |
| C1.5 | `commands/retention_history.rs` | `retention_history` |
| C1.6 | `commands/project_conversations.rs` | `project_conversations_list` |

### Round 2 — 2-command happy-path

All happy-path except `permissions` (no_state — `#[klynt_raw_command]`).

| Task | Module | Commands | Macro |
|---|---|---|---|
| C2.1 | `commands/integrations.rs` | `ai_tools_detect`, `ai_tools_install` | `#[klynt_command]` |
| C2.2 | `commands/knowledge_health.rs` | `knowledge_health_summary`, `knowledge_topic_detail` | `#[klynt_command]` |
| C2.3 | `commands/permissions.rs` | `permissions_check_accessibility`, `permissions_open_accessibility` | `#[klynt_raw_command]` (no state) |
| C2.4 | `commands/project_memories.rs` | `project_memories_list`, `project_memories_by_type` | `#[klynt_command]` |
| C2.5 | `commands/shortcuts.rs` | `shortcuts_get`, `shortcuts_update` | `#[klynt_command]` |
| C2.6 | `commands/oauth_commands.rs` (file at `oauth/commands.rs`) | `mcp_oauth_start`, `mcp_oauth_disconnect` | `#[klynt_command]` |

### Round 3 — 3-command happy-path

All happy-path. 8 modules.

| Task | Module |
|---|---|
| C3.1 | `commands/entities.rs` |
| C3.2 | `commands/entity_links.rs` |
| C3.3 | `commands/fabric.rs` |
| C3.4 | `commands/journey.rs` |
| C3.5 | `commands/pending_memory.rs` |
| C3.6 | `commands/project_sources.rs` |
| C3.7 | `commands/view.rs` |
| C3.8 | `commands/workspace.rs` |

### Round 4 — 4-command happy-path + `window` outlier

| Task | Module | Note |
|---|---|---|
| C4.1 | `commands/objectives.rs` | Happy-path |
| C4.2 | `commands/key_results.rs` | Happy-path |
| C4.3 | `commands/window.rs` | All 4 outliers — `#[klynt_raw_command]` (sync, no_state) |

### Round 5 — 5–9 command happy-path + simple outliers

| Task | Module | Note |
|---|---|---|
| C5.1 | `commands/areas.rs` | Happy-path |
| C5.2 | `commands/groups.rs` | Happy-path |
| C5.3 | `commands/reforge.rs` | Happy-path |
| C5.4 | `commands/distraction.rs` | 2 happy + 3 raw (rename_all_snake on 2; FocusTimer state on 1) |
| C5.5 | `commands/agents.rs` | Happy-path |
| C5.6 | `commands/annotations.rs` | Happy-path |
| C5.7 | `commands/capture.rs` | Happy-path |
| C5.8 | `commands/focus.rs` | 4 happy + 2 raw (no_state on 2) |
| C5.9 | `commands/language.rs` | Happy-path |
| C5.10 | `commands/voice.rs` | Happy-path |
| C5.11 | `commands/cron.rs` | Happy-path |
| C5.12 | `commands/practice.rs` | Happy-path |
| C5.13 | `commands/squads.rs` | Happy-path |
| C5.14 | `commands/autotuner.rs` | Happy-path |
| C5.15 | `commands/columns.rs` | Happy-path |
| C5.16 | `commands/voice_conversation.rs` | Happy-path |
| C5.17 | `commands/settings.rs` | Happy-path |
| C5.18 | `commands/workflows.rs` | Happy-path |
| C5.19 | `commands/chat.rs` | Happy-path |
| C5.20 | `commands/mirror.rs` | Happy-path |
| C5.21 | `commands/work_context.rs` | Happy-path |

### Round 6 — Mid-size + complex-outlier modules

| Task | Module | Note |
|---|---|---|
| C6.1 | `commands/launcher.rs` | 6 happy + 4 raw (no_state on 4) |
| C6.2 | `commands/tasks.rs` | All 17 happy-path; biggest mostly-clean module so far |
| C6.3 | `commands/coding_memory.rs` | 17 happy + 9 raw (different_error: 9 commands return `Result<T, String>` not `ApiError` — `#[klynt_raw_command]`) |

### Round 7 — The big three

| Task | Module | Note |
|---|---|---|
| C7.1 | `commands/cognitive.rs` | All 31 happy-path |
| C7.2 | `commands/finance.rs` | 33 happy + 5 raw (rename_all_camel on 5) |
| C7.3 | `commands/productivity.rs` | 29 happy + 18 raw (rename_all_snake on 8; FocusTimer different_state on 6; FocusTimer-only no_state on 4) |
| C7.4 | `commands/notes.rs` | 76 happy + 1 raw (`note_insight_tab_chat` — non-standard param order) |

### Phase C verification checkpoint

After each round's tasks land:

- [ ] Run `cargo nextest run -p desktop` — all tests pass including `no_double_registration`.
- [ ] Run `cargo nextest run -p desktop registration_drift` — TBD until Phase D wires the test; for now, manually verify `KLYNT_COMMANDS` slice and `collect_commands![...]` list intersect identically by counting names.
- [ ] Run `cargo tauri dev` and exercise the migrated modules' UI surfaces.
- [ ] PR: one PR per round (C1, C2, C3, C4, C5, C6, C7) for review-friendliness.

---

## Phase D — Switchover (4 tasks)

### Task D1: Add the `registration_drift` test

**Files:**
- Create: `crates/desktop/tests/registration_drift.rs`

- [ ] **Step 1: Write the test**

```rust
//! Asserts the linkme slice (Tauri runtime truth) and the specta hand-list
//! (FE binding truth) contain the same set of command names.

use std::collections::BTreeSet;
use desktop::specta_builder::{KLYNT_COMMANDS, build_specta};

#[test]
fn linkme_and_specta_lists_match() {
    let linkme: BTreeSet<&str> = KLYNT_COMMANDS.iter().map(|c| c.name).collect();

    let specta_handler = build_specta();
    let specta: BTreeSet<&str> = specta_handler
        .commands_iter()  // verify exact API per Plan 5 / pre-impl spike #3
        .map(|c| c.name)
        .collect();

    let missing: Vec<&&str> = linkme.difference(&specta).collect();
    let extra: Vec<&&str> = specta.difference(&linkme).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "Registration drift!\n  In linkme but not specta (add to collect_commands!): {missing:?}\n  In specta but not linkme (remove from collect_commands!): {extra:?}"
    );
}
```

- [ ] **Step 2: If `commands_iter()` doesn't exist on `tauri_specta::Builder`**

Fall back to a sibling `pub const SPECTA_COMMAND_NAMES: &[&str]` in `specta_builder.rs`, hand-maintained alongside `collect_commands![...]`. The drift test compares against this const. Slightly more fragile but still drift-checked.

- [ ] **Step 3: Run the test**

Run: `cargo nextest run -p desktop linkme_and_specta_lists_match`
Expected: pass (after Phase C, both surfaces have all 465 commands).

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/tests/registration_drift.rs
git commit -m "test(desktop): add registration_drift test (linkme vs specta lists)"
```

---

### Task D2: Replace dual-dispatcher with `klynt_invoke_handler` only

**Files:**
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Verify the legacy `generate_handler![...]` list is empty**

Run: `grep -A2 "generate_handler!" crates/desktop/src/main.rs`
Expected: `tauri::generate_handler![]` (empty bracketed list — Phase C dropped every name).

- [ ] **Step 2: Replace the dual-dispatcher with the simple form**

Before:

```rust
.invoke_handler({
    let klynt = crate::specta_builder::klynt_invoke_handler();
    let legacy = tauri::generate_handler![];

    move |invoke| {
        let name = invoke.message.command();
        if crate::specta_builder::KLYNT_COMMANDS.iter().any(|c| c.name == name) {
            klynt(invoke)
        } else {
            legacy(invoke)
        }
    }
})
```

After:

```rust
.invoke_handler(crate::specta_builder::klynt_invoke_handler())
```

- [ ] **Step 3: Build + smoke test**

```bash
cargo build -p desktop
cargo tauri dev   # in another terminal
```

Expected: app launches, every command works.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "refactor(desktop): swap dual-dispatcher for klynt_invoke_handler only"
```

---

### Task D3: Run the full test suite

- [ ] **Step 1: Workspace-wide build + tests**

```bash
cargo build --workspace 2>&1 | tail -5
cargo nextest run --workspace 2>&1 | tail -10
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20
cargo fmt --all --check
```

Expected: all clean.

- [ ] **Step 2: FE checks**

```bash
cd desktop-ui && bun run typecheck && bun run test 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 3: No commit — verification only.**

---

### Task D4: Delete the now-tautological `no_double_registration` test

**Files:**
- Delete: `crates/desktop/tests/no_double_registration.rs`
- Modify: `crates/desktop/src/lib.rs` (or `main.rs`) — delete `LEGACY_COMMAND_NAMES` const

- [ ] **Step 1: Confirm `LEGACY_COMMAND_NAMES` is empty**

Run: `grep -A3 "LEGACY_COMMAND_NAMES" crates/desktop/src/`
Expected: const declaration with `&[]` (empty).

- [ ] **Step 2: Delete the const + the test file**

```bash
rm crates/desktop/tests/no_double_registration.rs
```

Edit `lib.rs`/`main.rs` to remove the const declaration.

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/lib.rs crates/desktop/tests/no_double_registration.rs
git commit -m "chore(desktop): drop LEGACY_COMMAND_NAMES + no_double_registration test (Phase 6 complete)"
```

---

## Phase E — Cleanup (8 tasks)

### Task E1: Replace `tauri_command_names()` body with linkme iter

**Files:**
- Modify: `crates/desktop/src/dev_server/mod.rs:195-225`

- [ ] **Step 1: Read the existing body**

Run: `sed -n '195,225p' crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 2: Replace with one-liner**

Before:

```rust
fn tauri_command_names() -> std::collections::BTreeSet<String> {
    let main_rs = include_str!("../main.rs");
    main_rs
        .lines()
        .filter_map(|line| /* parse commands::*::* paths */)
        .collect()
}
```

After:

```rust
fn tauri_command_names() -> std::collections::BTreeSet<String> {
    crate::specta_builder::KLYNT_COMMANDS
        .iter()
        .map(|c| c.name.to_string())
        .collect()
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/dev_server/mod.rs
git commit -m "refactor(dev_server): drive tauri_command_names from KLYNT_COMMANDS slice"
```

---

### Task E2: Delete `dev_command_names()` and the two coverage tests

**Files:**
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Delete `dev_command_names()` body**

Find the function in `dev_server/mod.rs`. Delete the function entirely (it aggregated per-module `DEV_COMMANDS`, which no longer exist).

- [ ] **Step 2: Delete `dev_server_covers_all_tauri_commands` and `dev_server_has_no_orphan_commands` tests**

In the test module of the same file (typically near the bottom), delete both `#[test]`-annotated functions.

- [ ] **Step 3: Build + verify other tests still pass**

```bash
cargo nextest run -p desktop
```

Expected: clean. The deleted tests are now redundant (linkme slice IS the truth).

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/dev_server/mod.rs
git commit -m "chore(dev_server): drop coverage tests + dev_command_names (linkme-driven now)"
```

---

### Task E3: Sweep for any remaining `pub(crate) const DEV_COMMANDS`

**Files:** any `crates/desktop/src/commands/*.rs` that still has `DEV_COMMANDS`.

- [ ] **Step 1: Find any remaining declarations**

Run: `rg -l "pub\(crate\) const DEV_COMMANDS" crates/desktop/src/`
Expected: 0 files (Phase C should have caught all 50). If any remain, this task removes them.

- [ ] **Step 2: For each remaining file, delete the const**

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`
Expected: clean.

- [ ] **Step 4: Commit (only if changes)**

```bash
git add crates/desktop/src/commands/
git commit -m "chore(desktop): drop residual DEV_COMMANDS arrays"
```

---

### Task E4: Add the anti-bypass test

**Files:**
- Create: `crates/desktop/tests/no_raw_tauri_command_outside_macros.rs`

- [ ] **Step 1: Write the test**

```rust
//! Hard CI gate: no raw `#[tauri::command]` may appear in `crates/desktop/src/commands/`
//! or `crates/desktop/src/oauth/` unless wrapped by `#[klynt_command]` or
//! `#[klynt_raw_command]`. Ensures the convention can't degrade silently.

#[test]
fn no_raw_tauri_command_outside_macros() {
    let dirs = [
        "crates/desktop/src/commands/",
        "crates/desktop/src/oauth/",
    ];

    for dir in &dirs {
        let output = std::process::Command::new("rg")
            .args(["-l", "#\\[tauri::command", dir])
            .output()
            .expect("rg available — install ripgrep if missing");
        let files: Vec<_> = String::from_utf8(output.stdout).unwrap().lines().map(String::from).collect();

        for file in &files {
            let content = std::fs::read_to_string(file).unwrap();
            for (i, line) in content.lines().enumerate() {
                if line.contains("#[tauri::command") {
                    let context = content
                        .lines()
                        .skip(i.saturating_sub(3))
                        .take(7)
                        .collect::<Vec<_>>()
                        .join("\n");
                    assert!(
                        context.contains("klynt_command") || context.contains("klynt_raw_command"),
                        "Raw #[tauri::command] in {file} at line {} — must be wrapped by #[klynt_command] or #[klynt_raw_command]",
                        i + 1
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run -p desktop no_raw_tauri_command_outside_macros`
Expected: pass — by Phase E every `#[tauri::command]` is inside a klynt-macro context.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/tests/no_raw_tauri_command_outside_macros.rs
git commit -m "test(desktop): add no_raw_tauri_command_outside_macros anti-bypass guard"
```

---

### Task E5: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Find the existing "DEV_COMMANDS gotcha" section**

Run: `grep -n "DEV_COMMANDS" CLAUDE.md`

- [ ] **Step 2: Replace with the new recipe**

Delete the old gotcha bullet and insert this section (after the "Conventions" section):

```markdown
## Adding a Tauri command (Plan 6)

The IPC surface is gated behind two attribute macros in `crates/desktop-macros/`. Direct `#[tauri::command]` is forbidden in `crates/desktop/src/commands/` and `crates/desktop/src/oauth/` (enforced by `no_raw_tauri_command_outside_macros` test).

### Use `#[klynt_command]` for the happy path

A command qualifies for `#[klynt_command]` if and only if it:
- is `pub async fn`,
- takes no `state` parameter (the macro injects it),
- returns a bare type `T` (not `Result<T, _>`, not `CommandResult<T>`),
- and uses `State<'_, Arc<AppCore>>`.

```rust
#[klynt_command]
pub async fn task_get(id: String) -> Option<TaskResponse> {
    state.task_get(id).await
}
```

### Use `#[klynt_raw_command]` otherwise

For sync commands, non-AppCore state, no-state commands, `rename_all` overrides, or commands that return non-`ApiError` Results:

```rust
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub fn resize_window(app: AppHandle, label: String, height: f64) { ... }
```

### Steps after adding a new command

1. **Add the function path to `collect_commands![...]` in `crates/desktop/src/specta_builder.rs::build_specta()`.** The `registration_drift` test fails until you do this.
2. **Run `cargo tauri dev` once** to regenerate `desktop-ui/src/bindings.ts`. The `bindings_are_current` test fails until you do this.
3. Commit. The two tests above are the only hand-verifiable steps; everything else is automatic.
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude): replace DEV_COMMANDS gotcha with Plan 6 recipe"
```

---

### Task E6–E8: Add remaining trybuild test cases

**Files:**
- Create: `crates/desktop-macros/tests/trybuild/pass/{with_arg,multiple_args,with_app_handle,unit_return,complex_return}.rs`
- Create: `crates/desktop-macros/tests/trybuild/fail/{missing_async,missing_pub,declared_state,result_return,command_result_return,applied_to_struct,missing_return_type}.rs` (and matching `.stderr` files)
- Create: `crates/desktop-macros/tests/trybuild/raw_pass/{raw_sync,raw_camel_case,raw_snake_case,raw_focus_timer_state,raw_permissions}.rs`
- Create: `crates/desktop-macros/tests/trybuild/raw_fail/raw_on_struct.rs` (and `.stderr`)

These are mechanical to write — each follows the spec Section 6 enumeration.

#### Task E6: Add 5 happy-path pass cases

(One sub-task per case; combined here for brevity.)

For each of `with_arg.rs`, `multiple_args.rs`, `with_app_handle.rs`, `unit_return.rs`, `complex_return.rs`:

- Write the file with the test scenario.
- Run: `cargo nextest run -p desktop-macros ui_pass`.
- Commit: `test(desktop-macros): add trybuild pass case <name>`.

Example for `with_arg.rs`:

```rust
mod crate_stubs { /* same stubs as minimal.rs */ }
use crate_stubs as crate_;

#[desktop_macros::klynt_command]
pub async fn task_get(id: String) -> i32 {
    state.do_thing(id).await
}

fn main() {}
```

#### Task E7: Add 7 fail cases

For each of `missing_async.rs`, `missing_pub.rs`, `declared_state.rs`, `result_return.rs`, `command_result_return.rs`, `applied_to_struct.rs`, `missing_return_type.rs`:

- Write the `.rs` with the invalid input.
- Run `cargo nextest run -p desktop-macros ui_fail` once — trybuild writes the `.stderr` snapshot.
- Inspect the snapshot; commit it.
- Commit: `test(desktop-macros): add trybuild fail case <name>`.

Example for `missing_async.rs`:

```rust
mod crate_stubs { /* stubs */ }
use crate_stubs as crate_;

#[desktop_macros::klynt_command]
pub fn ping() -> i32 { 42 }   // missing async

fn main() {}
```

Expected `.stderr` (snapshot):

```
error: klynt_command requires `pub async fn`
 --> tests/trybuild/fail/missing_async.rs:6:5
  |
6 | pub fn ping() -> i32 { 42 }
  |     ^^
```

#### Task E8: Add 5 raw-command pass cases + 1 raw fail case

Mechanical — each test exercises one outlier shape. Total ~12 trybuild cases added.

---

## Phase F — Final verification (3 tasks)

### Task F1: Workspace-wide green check

- [ ] **Step 1: Build**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 2: Format check**

Run: `cargo fmt --all --check`
Expected: clean.

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -30`
Expected: zero new warnings.

- [ ] **Step 4: Tests**

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: all pass (including: `bindings_are_current`, `registration_drift`, `no_raw_tauri_command_outside_macros`, all trybuild cases).

- [ ] **Step 5: Doctests**

Run: `cargo test --workspace --doc 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 6: FE checks**

Run: `cd desktop-ui && bun run typecheck && bun run test && bun run lint 2>&1 | tail -10`
Expected: clean (lint warnings ok, no new errors).

---

### Task F2: Manual end-to-end smoke test

- [ ] **Step 1: Launch the app**

In one terminal: `cd desktop-ui && bun run dev`
In another: `cargo tauri dev`

- [ ] **Step 2: Exercise every major surface**

- Tasks: create, edit, complete, delete (round 6).
- Notes: create, edit, search (round 7).
- Finance: view accounts, add transaction (round 7).
- Productivity: start focus session, end session (round 7).
- Launcher: search, run a script (round 6 outliers).
- Window: resize, quit and reopen (round 4 outlier).
- Permissions: check accessibility (round 2 outlier).

Expected: every surface works, no panics, no IPC errors.

- [ ] **Step 3: Open React Query devtools (Plan 1) and verify cache invalidations propagate as before.**

- [ ] **Step 4: Run a sqlite3 CLI write outside the app to verify Plan 4's fallback still works**

```bash
sqlite3 ~/.klyntbot/data.db "UPDATE tasks SET title='test-plan6' WHERE id=(SELECT id FROM tasks LIMIT 1)"
```

Expected: FE invalidates within 5s.

- [ ] **Step 5: No commit — verification only.**

---

### Task F3: Push and PR

- [ ] **Step 1: Confirm clean tree**

Run: `git status`
Expected: clean.

- [ ] **Step 2: Push**

Run: `git push origin HEAD`

- [ ] **Step 3: Open the PR series**

If using per-round PRs (recommended), each Phase C round is its own PR. The infrastructure (Phases A, B, D, E, F) ships as one umbrella PR or as smaller PRs per phase.

```bash
gh pr create --title "feat: typed command macros (Plan 6)" --body "$(cat <<'EOF'
## Summary

Replaces 465 hand-annotated Tauri command shells with two attribute macros — `#[klynt_command]` for the happy path and `#[klynt_raw_command]` for outliers. Auto-registers commands into a `linkme::distributed_slice` for runtime dispatch; specta `collect_commands![...]` retains the FE binding hand-list backed by a 50ms drift test.

## Test plan
- [ ] `cargo build --workspace` clean
- [ ] `cargo nextest run --workspace` green (incl. `bindings_are_current`, `registration_drift`, `no_raw_tauri_command_outside_macros`, ~13 trybuild cases)
- [ ] `cargo clippy --workspace --all-targets --all-features` zero new warnings
- [ ] `cargo fmt --all --check` clean
- [ ] `cd desktop-ui && bun run typecheck && bun run test && bun run lint` green
- [ ] Manual smoke: launch app, exercise tasks/notes/finance/productivity/launcher/window/permissions
- [ ] sqlite3 CLI write triggers Plan 4 fallback within 5s

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage:**

- ✅ §Architecture — new crate `desktop-macros`, dependency graph → Tasks A5, A6
- ✅ §Macro 1 happy-path — input/expansion/failure modes → Task A10
- ✅ §Macro 2 raw — input/expansion/failure mode → Task A11
- ✅ §Registration — `KLYNT_COMMANDS` slice + `klynt_invoke_handler` → Task A7
- ✅ §Migration — 5 phases A–E → Phases A, B, C, D, E
- ✅ §Pre-implementation verifications #1, #2, #3 → Tasks A1, A2, D1 fallback
- ✅ §Testing — trybuild, drift, anti-bypass, coexistence → Tasks A12, E6–E8, D1, A9, E4
- ✅ §Documentation — CLAUDE.md update → Task E5
- ✅ §Decision log — every decision implemented in some task; no orphans

**Placeholder scan:**

- All "TBD-shaped" items in the plan are explicit task references with concrete steps.
- "Apply common steps above" in Phase C tasks references B1's fully-spelled template — DRY but not vague.
- Round tables explicitly enumerate every module + macro choice + outlier note.
- Spike tasks (A1, A2) have explicit STOP conditions if verification fails.
- Task E6–E8 sub-tasks are templated mechanically; one example shown per sub-task type.

**Type consistency:**

- `CommandRegistration { name, invoke, source }` introduced in A7 referenced identically in A10, A11, D1, E1.
- `KLYNT_COMMANDS` slice referenced consistently across A7, A8, D1, D2, E1.
- `klynt_invoke_handler` introduced in A7 used in A8 (dual-dispatch wrap), D2 (final form).
- `LEGACY_COMMAND_NAMES` introduced in A9, dropped names in C, deleted in D4 — full lifecycle covered.
- Macro names `klynt_command` and `klynt_raw_command` consistent throughout.
- `__klynt_dispatch_<name>` and `__klynt_command_<name>` naming consistent across A10 and A11.

---

## Out-of-scope notes

- **`tauri::command(rename_all = "...")`-style attributes via `#[klynt_command(rename_all = ...)]`**. Spec decision γ — outliers go through raw command macro.
- **Auto-error-conversion (`KlyntbotError → ApiError`)**. AppCore handlers already return `Result<T, ApiError>`; recon confirmed.
- **Codegen of command bodies from AppCore method names**. Spec decision Q1 option B — body stays explicit.
- **Build-script-driven specta `collect_commands![...]` generation**. Spec out-of-scope §Out-of-scope; drift test is the alternative.
- **Migrating the `agent:*` event dispatcher**. Plan 5 already deferred this; Plan 6 doesn't expand scope.

---

## Definition of Done

- `cargo build --workspace` clean, zero new warnings.
- `cargo nextest run --workspace` green: 4 new tests added (`bindings_are_current` from Plan 5 still green, plus `registration_drift`, `no_raw_tauri_command_outside_macros`, ~13 trybuild cases).
- `cargo test --workspace --doc` clean.
- `cargo clippy --workspace --all-targets --all-features` zero new warnings.
- `cargo fmt --all --check` clean.
- `cd desktop-ui && bun run typecheck && bun run test && bun run lint` green.
- 465 commands annotated with `#[klynt_command]` or `#[klynt_raw_command]`. Verify: `rg -c "klynt_command|klynt_raw_command" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs | awk -F: '{s+=$2} END {print s}'` returns ≥ 465.
- 0 raw `#[tauri::command]` annotations remain in `crates/desktop/src/commands/` or `crates/desktop/src/oauth/`. Verify: `rg "^\\#\\[tauri::command" crates/desktop/src/commands/ crates/desktop/src/oauth/` returns 0 results outside macro-emitted code.
- 0 `pub(crate) const DEV_COMMANDS` declarations remain. Verify: `rg "pub\\(crate\\) const DEV_COMMANDS" crates/desktop/src/` returns empty.
- `dev_server_covers_all_tauri_commands` and `dev_server_has_no_orphan_commands` tests are deleted.
- `LEGACY_COMMAND_NAMES` const and `no_double_registration` test are deleted.
- `tauri::generate_handler!` no longer appears in `main.rs` (replaced by `klynt_invoke_handler()`).
- CLAUDE.md has the new "Adding a Tauri command (Plan 6)" recipe section.
- Manual smoke: every UI surface exercised in Task F2 works; sqlite3 CLI fallback (Plan 4) still triggers within 5s.
- All commits use conventional-commit format with the right scope.

---

## End of Plan 6

After Plan 6: every Tauri command in the codebase goes through one of two macros, registration is provably exhaustive on the runtime side and drift-checked on the FE-binding side. Adding a command requires writing the function and listing its path once in `specta_builder.rs`. The macro is bulletproof on misuse — strict refusal of malformed input on the happy-path, permissive trust on the escape hatch.

**Combined deliverables across Plans 1–6:**

- Plans 1–4: real-time data layer (TanStack Query, MCP bridge, Distiller events, fallback poller)
- Plan 5: typed Tauri IPC via tauri-specta (TS bindings auto-generated, drift-checked)
- **Plan 6 (this plan): typed command macros — single attribute per command, auto-registered, drift-checked, anti-bypass-enforced**
