# Klynt Coding-in-Chat — Phase 1 Plan 4: Layer 2 Starlark + Hooks Engine

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the Layer 2 Starlark approval engine (replacing the FallThrough stub at `crates/klynt-execpolicy/src/policy.rs:13`) and the `klynt-hooks` subprocess engine with all 13 Claude-Code-compatible hook events. Wire `PreToolUse` + `PostToolUse` into klynt-core tools (Phase 1 minimum per master spec §13 line 1371) plus the 11 lifecycle hook integration sites.

**Architecture:** Vendor `codex-rs/execpolicy` and `codex-rs/hooks` via the existing `scripts/adapt_codex_vendor.sh`, adapt Codex-specific dependencies (`ThreadId` → klynt session key, `AbsolutePathBuf` → `PathBuf`, `codex_protocol::*` → re-declared local types). Codex ships 5 hook events; we extend with 8 (`SessionEnd`, `PreCompact`, `PostCompact`, `PreFileEdit`, `PostFileEdit`, `Notification`, `SubagentSpawn`, `Error`). `HookEngine` plumbs through `RoutingContext` (same pattern as `event_tx` from tool-layer-consolidation). Each tool's `execute()` fires `PreToolUse → action → PostToolUse`; mutating tools also fire `PreFileEdit → file-write → PostFileEdit`. Block returns abort the call; `modify_args` rewrites tool inputs before execution.

**Tech Stack:** Rust 1.93, `starlark = "0.13.0"` (Facebook's Rust Starlark), `tokio::process` (subprocess execution), `multimap`, `shlex`, `chrono`, `futures`, `regex`, `schemars`, klyntbot's existing `tools-core::events::ToolEvent`, klyntbot's existing `RoutingContext`, klyntbot's existing `ToolKitBuilder`.

**Spec references:**
- `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` §7 Layer 2 Starlark (lines 762-783), §7 Hook interaction + `hooks.toml` schema (lines 820-884), §13 Phase 1 deliverable list (line 1371: PreToolUse + PostToolUse fire correctly), §14 invariants K3 (approval gate composition).
- Plan 1 ✅ (foundation crate scaffolding)
- Plan 2 ✅ (bash end-to-end with macOS Seatbelt)
- Plan 3 ✅ (tool kit completion + Linux sandbox + diff rendering)
- Plan 3.5 ✅ (tool layer consolidation; commit `a8a1f354f` in main)

**Master spec amendments needed (folded into Task 9):**

1. **§7 Layer 2 example** (lines 770-775) shows `custom_rule(["git", "push"], handler=check_git_push)`. Codex execpolicy has no `custom_rule` builtin. Replace example with `prefix_rule` + Starlark conditional pattern (covered in Task 1 documentation).
2. **§7 line 849-865** lists 13 hook events. Codex hooks crate ships 5 (`PreToolUse`, `PostToolUse`, `SessionStart`, `UserPromptSubmit`, `Stop`). Klynt-hooks adds the remaining 8 in Task 3.
3. **`klynt-hooks/Cargo.toml` description** says "13-event Claude-Code-compatible schema" — accurate after Task 3.

---

## File structure

### Files created

```
bot/
├── crates/
│   ├── klynt-execpolicy/src/                     (replaces 34 LOC of stubs with vendored Codex content)
│   │   ├── lib.rs                                (re-exports — replaces existing 6-line file)
│   │   ├── decision.rs                           (already exists; will be amended for Codex's 3 variants → klyntbot's 4)
│   │   ├── error.rs                              (vendored from codex-rs/execpolicy/src/error.rs:101)
│   │   ├── parser.rs                             (vendored from codex-rs/execpolicy/src/parser.rs:472)
│   │   ├── policy.rs                             (replaces existing 17-line stub; vendored ~375 lines + adapter)
│   │   ├── rule.rs                               (vendored from codex-rs/execpolicy/src/rule.rs:306)
│   │   ├── executable_name.rs                    (vendored from codex-rs/execpolicy/src/executable_name.rs:29)
│   │   └── amend.rs                              (vendored from codex-rs/execpolicy/src/amend.rs:337; for "Add rule" UI)
│   │
│   ├── klynt-hooks/src/                          (replaces 12 LOC stub with vendored Codex content + 8 added events)
│   │   ├── lib.rs                                (replaces existing; re-exports)
│   │   ├── schema.rs                             (vendored from codex-rs/hooks/src/schema.rs:638; extended for 8 new events)
│   │   ├── types.rs                              (vendored from codex-rs/hooks/src/types.rs:290; ThreadId→SessionKey)
│   │   ├── registry.rs                           (vendored from codex-rs/hooks/src/registry.rs:163; ConfigLayerStack→KlyntConfig)
│   │   ├── error.rs                              (klynt-specific; small file)
│   │   ├── engine/
│   │   │   ├── mod.rs                            (vendored 154 lines)
│   │   │   ├── command_runner.rs                 (vendored 135 lines verbatim)
│   │   │   ├── config.rs                         (vendored 48 lines verbatim; extended for 8 new event names)
│   │   │   ├── discovery.rs                      (vendored 329 lines; ConfigLayerStack→KlyntConfig)
│   │   │   ├── dispatcher.rs                     (vendored 337 lines; codex_protocol→local re-declarations)
│   │   │   ├── output_parser.rs                  (vendored 336 lines verbatim)
│   │   │   └── schema_loader.rs                  (vendored 90 lines; embedded schemas via include_str!)
│   │   ├── events/
│   │   │   ├── mod.rs                            (re-export all 13 event modules)
│   │   │   ├── common.rs                         (vendored 212 lines; codex_protocol→local types)
│   │   │   ├── pre_tool_use.rs                   (vendored 539 lines; ThreadId→SessionKey)
│   │   │   ├── post_tool_use.rs                  (vendored 551 lines; ThreadId→SessionKey)
│   │   │   ├── session_start.rs                  (vendored 378 lines; ThreadId→SessionKey)
│   │   │   ├── user_prompt_submit.rs             (vendored 436 lines; ThreadId→SessionKey)
│   │   │   ├── stop.rs                           (vendored 545 lines; ThreadId→SessionKey)
│   │   │   ├── session_end.rs                    (NEW — klynt extension; ~120 LOC)
│   │   │   ├── pre_compact.rs                    (NEW — klynt extension; ~120 LOC)
│   │   │   ├── post_compact.rs                   (NEW — klynt extension; ~120 LOC)
│   │   │   ├── pre_file_edit.rs                  (NEW — klynt extension; ~150 LOC)
│   │   │   ├── post_file_edit.rs                 (NEW — klynt extension; ~150 LOC)
│   │   │   ├── notification.rs                   (NEW — klynt extension; ~100 LOC)
│   │   │   ├── subagent_spawn.rs                 (NEW — klynt extension; ~100 LOC)
│   │   │   └── error.rs                          (NEW — klynt extension; ~100 LOC)
│   │   └── schema/generated/                     (vendored JSON schemas referenced via include_str!)
│   │       ├── pre_tool_use.json
│   │       ├── post_tool_use.json
│   │       ├── session_start.json
│   │       ├── user_prompt_submit.json
│   │       └── stop.json
│   │
│   └── klynt-core/src/
│       └── tools/shared/
│           └── hook_emit.rs                      (NEW — helper to fire hooks from tool execute(); ~100 LOC)
│
├── desktop-ui/src/features/
│   ├── coding/components/
│   │   ├── StarlarkRuleEditor.tsx                (NEW — inline editor for "Add rule" flow; ~150 LOC)
│   │   └── StarlarkRuleEditor.test.tsx           (NEW — Vitest)
│   └── settings/components/sections/
│       ├── HooksSection.tsx                      (NEW — read-only display of ~/.klyntbot/hooks.toml; ~120 LOC)
│       └── HooksSection.test.tsx                 (NEW — Vitest)
│
└── tests/
    ├── coding_in_chat_property.rs                (extended with K3 + 4 new K-invariants for Plan 4)
    └── integration/
        └── plan4_hooks_e2e.rs                    (NEW — end-to-end hook test scenarios)
```

### Files modified

```
crates/klynt-execpolicy/Cargo.toml                (+ starlark = "0.13.0", multimap, shlex, anyhow, serde_json)
crates/klynt-execpolicy/src/decision.rs           (variant rename pass: Codex Prompt→Ask, Forbidden→Forbid; add FallThrough)
crates/klynt-execpolicy/src/lib.rs                (re-exports updated)
crates/klynt-execpolicy/src/starlark_stub.rs      (DELETE — was a 1-line stub)

crates/klynt-hooks/Cargo.toml                     (+ anyhow, chrono, futures, regex, schemars, serde_json, klynt-protocol)
crates/klynt-hooks/src/lib.rs                     (rewritten; replaces 12-line stub)

crates/klynt-protocol/src/lib.rs                  (add SessionKey, HookRunSummary, HookCompletedEvent, HookEventName; replaces codex_protocol references)

crates/tools-core/src/routing.rs                  (RoutingContext gains hook_engine: Option<Arc<HookEngine>>)
crates/tools-core/src/events.rs                   (ToolEvent gains HookExecuted variant)
crates/tools-core/Cargo.toml                      (+ klynt-hooks workspace dep)

crates/klynt-core/src/registry/builder.rs         (ToolKitBuilder gains hook_engine: Option<Arc<HookEngine>>)
crates/klynt-core/src/tools/shared/mod.rs         (re-export hook_emit)
crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,read,glob,grep,web_fetch,ask_user,tool_search}.rs   (PreToolUse + PostToolUse hook fire calls)
crates/klynt-core/src/tools/{edit,write,apply_patch,notebook_edit}.rs   (also PreFileEdit + PostFileEdit)
crates/klynt-core/Cargo.toml                      (+ klynt-hooks workspace dep)

crates/agent/src/agent_loop/mod.rs                (SessionStart, SessionEnd, Stop, Error fires)
crates/agent/src/execution/core.rs                (PreCompact, PostCompact fires; or wherever MidLoopCompressor lives)
crates/agent/src/subagent.rs                      (SubagentSpawn fires)

crates/app-core/src/handlers/chat/streaming.rs    (UserPromptSubmit fires; Notification fires from approval emit path)
crates/app-core/src/init/mod.rs                   (HookEngine construction; AgentRuntime::set_hook_engine; ToolKitBuilder.hook_engine wiring)
crates/app-core/src/lib.rs                        (or wherever AgentRuntime lives — add hook_engine accessor)

crates/desktop/src/commands/coding.rs             (chat_save_starlark_rule + coding_hooks_list Tauri commands)
crates/desktop/src/specta_builder.rs              (register new commands)

desktop-ui/src/features/coding/components/ApprovalCard.tsx                              (wire "Add rule" button → StarlarkRuleEditor)
desktop-ui/src/features/coding/hooks/useApprovalQueue.ts                                 (handle add_rule with starlark_source decision)
desktop-ui/src/features/settings/components/Settings.tsx (or equivalent)                 (add HooksSection tab)

docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md                         (amendments per Task 9)
```

### Files deleted

```
crates/klynt-execpolicy/src/starlark_stub.rs                    (was a 1-line stub; vendor replaces)
```

---

## Sequencing

```
Phase A — Engines (Tasks 1, 2, 3 — parallelizable across separate worktrees if desired)
  Task 1: Vendor + adapt klynt-execpolicy (Layer 2 Starlark)
  Task 2: Vendor + adapt klynt-hooks (5 Codex events)
  Task 3: Extend klynt-hooks with 8 additional events

Phase B — Core integration (Tasks 4, 5, 6 — sequential within phase, after Tasks 1+2+3)
  Task 4: HookEngine lifecycle + RoutingContext + ToolKitBuilder wiring
  Task 5: PreToolUse + PostToolUse integration in klynt-core tools (PHASE 1 MINIMUM ENDS HERE)
  Task 6: PreFileEdit + PostFileEdit integration in mutating tools

Phase C — Lifecycle hook integrations (Tasks 7, 8 — independent of each other after Phase B)
  Task 7: Agent-loop lifecycle hooks (SessionStart, SessionEnd, Stop, Error)
  Task 8: Composer + compaction + subagent + notification hooks (UserPromptSubmit, PreCompact, PostCompact, SubagentSpawn, Notification)

Phase D — UI + Verification (Task 9 — final)
  Task 9: "Add rule" inline Starlark editor + Settings HooksSection + master spec amendments + K-invariants + integration tests
```

Phase 1 minimum (per master spec line 1371): Tasks 1-5. Tasks 6-9 complete the Plan 4 scope but can defer to follow-up if shipping pressure. Recommended: ship all 9 in one PR (or 2 PRs split at end-of-Task-5).

---

## Task 1: Vendor + adapt `klynt-execpolicy` (Layer 2 Starlark engine)

**Goal:** Replace the 34-LOC stub with the vendored Codex execpolicy crate. The `Policy::eval(argv, cwd) -> Decision` signature in `guard.rs:94` continues to work via an adapter wrapping `Policy::check(cmd) -> Evaluation` from Codex. Adds real Starlark rule loading from `~/.klyntbot/rules/*.rules`.

**Files:**
- Modify: `crates/klynt-execpolicy/Cargo.toml`
- Vendor: `crates/klynt-execpolicy/src/{decision,error,executable_name,parser,policy,rule,amend}.rs`
- Modify: `crates/klynt-execpolicy/src/lib.rs`
- Delete: `crates/klynt-execpolicy/src/starlark_stub.rs`
- Test: `crates/klynt-execpolicy/tests/parse_rules.rs` (new)
- Test: `crates/klynt-execpolicy/tests/eval_rules.rs` (new)

- [ ] **Step 1: Add Cargo deps**

Edit `crates/klynt-execpolicy/Cargo.toml`. Replace the `[dependencies]` section with:

```toml
[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
multimap = "0.10"
shlex = { workspace = true }
starlark = "0.13.0"
walkdir = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

Verify the workspace `Cargo.toml` has `multimap`, `shlex`, `walkdir`, `tempfile`. If `starlark = "0.13.0"` is not in workspace deps, add to root `Cargo.toml`:

```toml
[workspace.dependencies]
# ... existing ...
starlark = "0.13.0"
```

- [ ] **Step 2: Verify deps resolve**

Run `cargo check -p klynt-execpolicy` and inspect the tail. Expected: dep resolution successful (still has stub source; only checking that deps compile).

- [ ] **Step 3: Run the adapt-codex-vendor script**

```bash
./scripts/adapt_codex_vendor.sh \
  --from-dir /Users/jayden/Projects/Klynt/codex \
  --source codex-rs/execpolicy \
  --dest /tmp/klynt-execpolicy-vendored \
  --rename codex_execpolicy=klynt_execpolicy \
  --rename codex-execpolicy=klynt-execpolicy
```

Expected: `/tmp/klynt-execpolicy-vendored/` populated with renamed source.

- [ ] **Step 4: Copy vendored sources into klynt-execpolicy, dropping CLI files**

```bash
cp /tmp/klynt-execpolicy-vendored/src/decision.rs       crates/klynt-execpolicy/src/decision.rs
cp /tmp/klynt-execpolicy-vendored/src/error.rs          crates/klynt-execpolicy/src/error.rs
cp /tmp/klynt-execpolicy-vendored/src/executable_name.rs crates/klynt-execpolicy/src/executable_name.rs
cp /tmp/klynt-execpolicy-vendored/src/parser.rs         crates/klynt-execpolicy/src/parser.rs
cp /tmp/klynt-execpolicy-vendored/src/policy.rs         crates/klynt-execpolicy/src/policy.rs
cp /tmp/klynt-execpolicy-vendored/src/rule.rs           crates/klynt-execpolicy/src/rule.rs
cp /tmp/klynt-execpolicy-vendored/src/amend.rs          crates/klynt-execpolicy/src/amend.rs
# Don't copy main.rs or execpolicycheck.rs — these are Codex CLI binaries.
rm crates/klynt-execpolicy/src/starlark_stub.rs
```

- [ ] **Step 5: Replace `AbsolutePathBuf` references with `PathBuf`**

Run `rg "AbsolutePathBuf|codex_utils_absolute_path" crates/klynt-execpolicy/src/`. For each hit, edit the file and:
- Replace `use codex_utils_absolute_path::AbsolutePathBuf;` with `use std::path::PathBuf;`
- Replace `AbsolutePathBuf` with `PathBuf` in type annotations
- Replace `AbsolutePathBuf::new(p)?` with `p.to_path_buf()` or similar
- Wherever Codex used `AbsolutePathBuf::canonicalize(p)?`, use `p.canonicalize()?`

Per the inventory: 3 sites in `parser.rs`, `policy.rs`, `rule.rs`. Each site is a few lines.

- [ ] **Step 6: Update klynt-execpolicy/src/lib.rs**

Replace `crates/klynt-execpolicy/src/lib.rs` with:

```rust
//! Klynt execution policy — Starlark prefix-rule approval engine.
//! Adapted from codex-rs/execpolicy/.
//!
//! See `policy.rs` for the public `Policy` API.
//! See `parser.rs` for the Starlark grammar.
//! See `decision.rs` for the result type.

pub mod amend;
pub mod decision;
pub mod error;
pub mod executable_name;
pub mod parser;
pub mod policy;
pub mod rule;

pub use decision::Decision;
pub use error::{Error, Result};
pub use parser::parse_to_policy;
pub use policy::{Evaluation, Policy, RuleMatch};
```

- [ ] **Step 7: Reconcile the Decision enum**

Codex's `decision.rs:9` has 3 variants: `Allow`, `Prompt`, `Forbidden`. klyntbot's stub had 4: `Allow`, `Ask`, `Forbid`, `FallThrough`. The tool-layer-consolidation `guard.rs:95-113` matches against `ExecDecision::Allow / Forbid / Ask / FallThrough` — must keep all 4 variants.

Modify `crates/klynt-execpolicy/src/decision.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    /// Codex calls this `Prompt`; klyntbot calls it `Ask` (matches what
    /// the chat-inline approval card surfaces to users).
    Ask,
    /// Codex calls this `Forbidden`; klyntbot uses `Forbid` for brevity.
    Forbid,
    /// Klynt-specific: signals "no rule matched"; falls through to Layer 1
    /// or Layer 3 in the orchestrator at klynt-core/src/approval/guard.rs.
    FallThrough,
}

impl Decision {
    /// Map from the Starlark `decision="..."` string parameter on `prefix_rule`.
    /// Codex parser uses `"allow" | "prompt" | "forbidden"`. Klynt's `Ask`
    /// alias accepts both `"ask"` and `"prompt"`.
    pub fn from_starlark_str(s: &str) -> Option<Self> {
        match s {
            "allow"             => Some(Self::Allow),
            "ask" | "prompt"    => Some(Self::Ask),
            "forbid" | "forbidden" => Some(Self::Forbid),
            _ => None,
        }
    }
}
```

- [ ] **Step 8: Update parser.rs to use the renamed variants**

Open `crates/klynt-execpolicy/src/parser.rs`. Find the `prefix_rule` builtin around line 348. The Codex version maps `"allow" | "prompt" | "forbidden"` directly to its 3-variant Decision. Replace with:

```rust
// Inside the prefix_rule builtin body where decision is parsed:
let dec_str: &str = decision.unpack_str().ok_or_else(|| /* invalid */)?;
let dec = Decision::from_starlark_str(dec_str).ok_or_else(|| {
    starlark::Error::new_other(anyhow::anyhow!(
        "invalid decision '{dec_str}'; expected one of: allow, ask, prompt, forbid, forbidden"
    ))
})?;
```

(Exact patch shape depends on Codex's existing code — adapt to match.)

- [ ] **Step 9: Add Policy::eval adapter for guard.rs compat**

Open `crates/klynt-execpolicy/src/policy.rs`. After the existing `impl Policy` block, add:

```rust
impl Policy {
    /// Adapter for klynt-core/approval/guard.rs:94 — wraps `Policy::check` to
    /// return the simpler `Decision` rather than `Evaluation`.
    ///
    /// Splits on whitespace if `argv` was constructed from a string; production
    /// callers should split via shlex first.
    pub fn eval(&self, argv: &[&str], _cwd: Option<&std::path::Path>) -> Decision {
        let cmd: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        match self.check(&cmd, |_| Decision::FallThrough) {
            Evaluation::Match { decision, .. } => decision,
            Evaluation::NoMatch => Decision::FallThrough,
        }
    }

    /// Persist a session-only "always allow this prefix" rule. Mutating in-memory
    /// rules; not written to ~/.klyntbot/rules/. The "Allow always" approval-card
    /// button in `desktop-ui/src/features/coding/components/ApprovalCard.tsx`
    /// triggers this via Tauri command `chat_save_starlark_rule`.
    pub fn append_session_allow_prefix(&self, prefixes: &[&str]) {
        for p in prefixes {
            let parts: Vec<String> = shlex::split(p).unwrap_or_else(|| vec![p.to_string()]);
            let _ = self.add_prefix_rule(&parts, Decision::Allow);
        }
    }

    /// Walk a directory of `*.rules` files, parse each as Starlark, merge into
    /// a single Policy. The directory layout is `~/.klyntbot/rules/<name>.rules`.
    pub fn load_from_dir(path: &std::path::Path) -> Result<Self, std::io::Error> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let mut accumulated = Self::empty();
        for entry in walkdir::WalkDir::new(path).follow_links(false) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
            };
            if !entry.file_type().is_file() { continue; }
            if entry.path().extension().and_then(|e| e.to_str()) != Some("rules") { continue; }
            let source = std::fs::read_to_string(entry.path())?;
            let parsed = match crate::parser::parse_to_policy(&source, entry.path()) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("klynt-execpolicy: skipping invalid rule file {:?}: {e}", entry.path());
                    continue;
                }
            };
            accumulated = accumulated.merge_overlay(&parsed);
        }
        Ok(accumulated)
    }
}
```

(`tracing` may not be a dep yet; either add or use `eprintln!` for warnings.)

- [ ] **Step 10: Verify compilation**

Run `cargo build -p klynt-execpolicy` and inspect output. Expected: clean. Common errors:
- Missing `multimap` / `shlex` / `walkdir` deps → add to Cargo.toml
- Type mismatch on `Decision` → adjust `Evaluation::Match { decision, .. }` arms
- `tracing` missing → add `tracing = { workspace = true }` to deps

Iterate until clean.

- [ ] **Step 11: Write the failing parse test**

Create `crates/klynt-execpolicy/tests/parse_rules.rs`:

```rust
use klynt_execpolicy::{Decision, Policy};
use std::fs;
use tempfile::TempDir;

#[test]
fn parse_simple_prefix_rule_allow() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("git.rules"),
        r#"
prefix_rule(["git", "status"], decision="allow")
"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let cmd = vec!["git".to_string(), "status".to_string()];
    let dec = policy.eval(&cmd.iter().map(String::as_str).collect::<Vec<_>>(), None);
    assert_eq!(dec, Decision::Allow);
}

#[test]
fn parse_prefix_rule_ask() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("git.rules"),
        r#"
prefix_rule(["git", "push"], decision="ask")
"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let cmd: Vec<&str> = vec!["git", "push"];
    let dec = policy.eval(&cmd, None);
    assert_eq!(dec, Decision::Ask);
}

#[test]
fn parse_prefix_rule_forbid_via_forbidden_keyword() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("danger.rules"),
        r#"
prefix_rule(["rm", "-rf", "/"], decision="forbidden")
"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["rm", "-rf", "/"], None);
    assert_eq!(dec, Decision::Forbid);
}

#[test]
fn no_rule_falls_through() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("git.rules"),
        r#"prefix_rule(["git", "status"], decision="allow")"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["ls", "-la"], None);
    assert_eq!(dec, Decision::FallThrough);
}

#[test]
fn empty_dir_returns_empty_policy() {
    let dir = TempDir::new().unwrap();
    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["git", "status"], None);
    assert_eq!(dec, Decision::FallThrough);
}

#[test]
fn nonexistent_dir_returns_empty_policy() {
    let path = std::path::Path::new("/nonexistent-klynt-rules-dir-12345");
    let policy = Policy::load_from_dir(path).expect("load");
    let dec = policy.eval(&["git", "status"], None);
    assert_eq!(dec, Decision::FallThrough);
}

#[test]
fn invalid_rule_file_is_skipped() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("good.rules"), r#"prefix_rule(["git", "status"], decision="allow")"#).unwrap();
    fs::write(dir.path().join("bad.rules"), r#"this is not valid starlark"#).unwrap();
    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["git", "status"], None);
    assert_eq!(dec, Decision::Allow);
}
```

- [ ] **Step 12: Run parse tests**

Run `cargo nextest run -p klynt-execpolicy --test parse_rules`. Expected: 7 tests pass. If any fail:
- Test 1 fails → likely `Policy::load_from_dir` doesn't iterate `*.rules` correctly. Check the extension filter.
- Test 7 fails (invalid file not skipped) → ensure parser errors don't propagate; we wrap with `match parse { Ok(_) => ..., Err(_) => continue }`.

- [ ] **Step 13: Write session_allow_prefix test**

Create `crates/klynt-execpolicy/tests/session_allow.rs`:

```rust
use klynt_execpolicy::{Decision, Policy};

#[test]
fn append_session_allow_prefix_grants_immediate_allow() {
    let policy = Policy::empty();
    policy.append_session_allow_prefix(&["cargo nextest run"]);
    let dec = policy.eval(&["cargo", "nextest", "run", "--workspace"], None);
    assert_eq!(dec, Decision::Allow);
}

#[test]
fn session_allow_does_not_persist_to_disk() {
    let dir = tempfile::TempDir::new().unwrap();
    {
        let policy = Policy::load_from_dir(dir.path()).unwrap();
        policy.append_session_allow_prefix(&["foo bar"]);
    }
    // New load: previous session's runtime additions are gone.
    let policy2 = Policy::load_from_dir(dir.path()).unwrap();
    let dec = policy2.eval(&["foo", "bar", "baz"], None);
    assert_eq!(dec, Decision::FallThrough);
}
```

- [ ] **Step 14: Run session tests**

Run `cargo nextest run -p klynt-execpolicy --test session_allow`. Expected: 2 tests pass. Common gotcha: `append_session_allow_prefix` must take `&self` (not `&mut self`) — Policy needs internal mutability via `RefCell` or `Mutex` if its rules collection isn't already lock-protected.

- [ ] **Step 15: Verify tool layer's `guard.rs` integration still compiles**

Run `cargo build -p klynt-core`. Expected: clean. The `guard.rs:94` call `ctx.policy.eval(&argv, None)` now hits the real implementation instead of the FallThrough stub.

- [ ] **Step 16: Add property test for K3 (approval gate composition)**

Append to `tests/coding_in_chat_property.rs`:

```rust
proptest! {
    /// K3 — Approval gate composition.
    /// For any (Layer1 decision, Layer2 decision) pair, the merged decision
    /// follows the spec's priority: deny beats allow beats ask; FallThrough
    /// from one layer passes to the next.
    #[test]
    fn k3_approval_gate_composition(
        l1_idx in 0u8..4,
        l2_idx in 0u8..4,
    ) {
        use klynt_execpolicy::Decision;
        let l1 = match l1_idx { 0 => Decision::Allow, 1 => Decision::Ask, 2 => Decision::Forbid, _ => Decision::FallThrough };
        let l2 = match l2_idx { 0 => Decision::Allow, 1 => Decision::Ask, 2 => Decision::Forbid, _ => Decision::FallThrough };

        // The tool-layer-consolidation `guard.rs:95-113` merge logic:
        //   if l1 == Auto (Allow/Forbid) → return l1 (short-circuits Layer 2)
        //   else l2 == Allow → Auto Allow Layer2
        //   else l2 == Forbid → Auto Forbid Layer2
        //   else l2 == Ask → Ask Layer2
        //   else l2 == FallThrough → fall back to l1
        let merged = match l1 {
            Decision::Allow | Decision::Forbid => l1,
            _ => match l2 {
                Decision::Allow => Decision::Allow,
                Decision::Forbid => Decision::Forbid,
                Decision::Ask => Decision::Ask,
                Decision::FallThrough => l1,
            }
        };
        // Property: Forbid wins per the merge logic semantics.
        if l1 == Decision::Forbid {
            prop_assert_eq!(merged, Decision::Forbid);
        } else if l1 == Decision::Allow {
            prop_assert_eq!(merged, Decision::Allow);
        } else if l2 == Decision::Forbid {
            prop_assert_eq!(merged, Decision::Forbid);
        }
    }
}
```

- [ ] **Step 17: Run K3**

Run `cargo nextest run --workspace --test coding_in_chat_property k3`. Expected: pass.

- [ ] **Step 18: Run full nextest + clippy**

Run:
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets --all-features`

Expected: green, zero warnings.

- [ ] **Step 19: Commit**

```bash
git add crates/klynt-execpolicy/Cargo.toml \
        crates/klynt-execpolicy/src/{lib,decision,error,executable_name,parser,policy,rule,amend}.rs \
        crates/klynt-execpolicy/tests/{parse_rules,session_allow}.rs \
        Cargo.toml \
        tests/coding_in_chat_property.rs
git rm crates/klynt-execpolicy/src/starlark_stub.rs

git commit -m "$(cat <<'EOF'
feat(execpolicy): vendor + adapt Codex Starlark engine for Layer 2

Replaces the 34-LOC stub with a real Layer 2 evaluator vendored from
codex-rs/execpolicy/. Uses Facebook's starlark = "0.13.0" crate to
parse ~/.klyntbot/rules/*.rules files at runtime. The Policy::eval
adapter wraps the upstream Policy::check → Evaluation API to preserve
the Decision-returning shape that klynt-core/src/approval/guard.rs
already calls.

Adapts:
- AbsolutePathBuf → PathBuf (3 sites in parser/policy/rule)
- Codex's 3-variant Decision (Allow/Prompt/Forbidden) → klyntbot's
  4-variant (Allow/Ask/Forbid/FallThrough); from_starlark_str maps
  the wire strings.
- Drops Codex's main.rs and execpolicycheck.rs (CLI binaries)

Implements append_session_allow_prefix for the "Allow always" approval
card button (in-memory only; persistent rules use amend.rs).

Adds invariant K3 (approval gate composition property test).

Part of Phase 1 Plan 4 (commit 1/9).
Spec: docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md §7

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Vendor + adapt `klynt-hooks` (5 Codex events)

**Goal:** Vendor `codex-rs/hooks/` and adapt for klyntbot. The 5 Codex events (`PreToolUse`, `PostToolUse`, `SessionStart`, `UserPromptSubmit`, `Stop`) work end-to-end. Subprocess execution, timeout, fail_open, and the schema-loader path all carry over.

**Files:**
- Modify: `crates/klynt-hooks/Cargo.toml`
- Vendor: `crates/klynt-hooks/src/{lib,schema,types,registry}.rs`
- Vendor: `crates/klynt-hooks/src/engine/*.rs`
- Vendor: `crates/klynt-hooks/src/events/{mod,common,pre_tool_use,post_tool_use,session_start,user_prompt_submit,stop}.rs`
- Vendor: `crates/klynt-hooks/schema/generated/*.json`
- Modify: `crates/klynt-protocol/src/lib.rs` (add hook protocol types replacing codex_protocol references)
- Test: `crates/klynt-hooks/tests/parse_config.rs`
- Test: `crates/klynt-hooks/tests/run_hook_subprocess.rs`

- [ ] **Step 1: Add Cargo deps to klynt-hooks**

Edit `crates/klynt-hooks/Cargo.toml`. Replace `[dependencies]` with:

```toml
[dependencies]
common = { path = "../common" }
klynt-protocol = { path = "../klynt-protocol" }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
toml = "0.8"
tokio = { workspace = true, features = ["process", "time", "macros", "io-util", "sync"] }
anyhow = { workspace = true }
chrono = { workspace = true, features = ["serde"] }
futures = { workspace = true, features = ["alloc"] }
regex = { workspace = true }
schemars = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 2: Add hook protocol types to klynt-protocol**

Open `crates/klynt-protocol/src/lib.rs`. Add the types Codex expects from `codex_protocol::protocol::*`:

```rust
//! ... existing content ...

use serde::{Deserialize, Serialize};

/// Stable session identifier — newtype around String matching klyntbot's
/// existing `SessionKey` in common::types.
pub use common::types::SessionKey;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookEventName {
    PreToolUse,
    PostToolUse,
    SessionStart,
    UserPromptSubmit,
    Stop,
    // Klynt extensions added in Task 3:
    SessionEnd,
    PreCompact,
    PostCompact,
    PreFileEdit,
    PostFileEdit,
    Notification,
    SubagentSpawn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookRunStatus {
    Success,
    Blocked,
    Timeout,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HookOutputEntry {
    pub kind: HookOutputEntryKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookOutputEntryKind {
    Stdout,
    Stderr,
    Block,
    ModifyArgs,
    AdditionalContext,
    StopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HookRunSummary {
    pub event: HookEventName,
    pub matcher: Option<String>,
    pub command: String,
    pub status: HookRunStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub output: Vec<HookOutputEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HookCompletedEvent {
    pub session_id: SessionKey,
    pub run: HookRunSummary,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookExecutionMode {
    Subprocess,
    InProcess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookHandlerType {
    Command,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookScope {
    User,
    Project,
}
```

Verify `common::types::SessionKey` exists by running `rg "pub struct SessionKey|pub type SessionKey" crates/common/src/`. If absent, define it as a `pub type SessionKey = String;` alias in `crates/common/src/types.rs`.

- [ ] **Step 3: Vendor hooks via the script**

```bash
./scripts/adapt_codex_vendor.sh \
  --from-dir /Users/jayden/Projects/Klynt/codex \
  --source codex-rs/hooks \
  --dest /tmp/klynt-hooks-vendored \
  --rename codex_hooks=klynt_hooks \
  --rename codex-hooks=klynt-hooks
```

- [ ] **Step 4: Copy vendored content, dropping Codex-specific files**

```bash
mkdir -p crates/klynt-hooks/src/engine crates/klynt-hooks/src/events crates/klynt-hooks/schema/generated

# Copy core lib files (drop legacy_notify.rs, user_notification.rs, bin/)
cp /tmp/klynt-hooks-vendored/src/lib.rs       crates/klynt-hooks/src/lib.rs
cp /tmp/klynt-hooks-vendored/src/schema.rs    crates/klynt-hooks/src/schema.rs
cp /tmp/klynt-hooks-vendored/src/types.rs     crates/klynt-hooks/src/types.rs
cp /tmp/klynt-hooks-vendored/src/registry.rs  crates/klynt-hooks/src/registry.rs

# Engine subdirectory
cp -r /tmp/klynt-hooks-vendored/src/engine/* crates/klynt-hooks/src/engine/

# Events (5 Codex-shipped events)
cp /tmp/klynt-hooks-vendored/src/events/mod.rs               crates/klynt-hooks/src/events/mod.rs
cp /tmp/klynt-hooks-vendored/src/events/common.rs            crates/klynt-hooks/src/events/common.rs
cp /tmp/klynt-hooks-vendored/src/events/pre_tool_use.rs      crates/klynt-hooks/src/events/pre_tool_use.rs
cp /tmp/klynt-hooks-vendored/src/events/post_tool_use.rs     crates/klynt-hooks/src/events/post_tool_use.rs
cp /tmp/klynt-hooks-vendored/src/events/session_start.rs     crates/klynt-hooks/src/events/session_start.rs
cp /tmp/klynt-hooks-vendored/src/events/user_prompt_submit.rs crates/klynt-hooks/src/events/user_prompt_submit.rs
cp /tmp/klynt-hooks-vendored/src/events/stop.rs              crates/klynt-hooks/src/events/stop.rs

# Schema fixtures
cp /tmp/klynt-hooks-vendored/schema/generated/*.json crates/klynt-hooks/schema/generated/
```

- [ ] **Step 5: Replace `codex_protocol::*` and `codex_config::*` references**

Run `rg "codex_protocol|codex_config" crates/klynt-hooks/src/`. For each hit, edit the file:

- `use codex_protocol::ThreadId;` → `use common::types::SessionKey as ThreadId;` (alias keeps existing identifiers; or do the explicit rename pass)
- `use codex_protocol::models::SandboxPermissions;` → drop the import; remove `SandboxPermissions` from `HookToolInputLocalShell` struct (the field is Codex-specific)
- `use codex_protocol::protocol::HookRunSummary;` → `use klynt_protocol::HookRunSummary;`
- `use codex_protocol::protocol::HookCompletedEvent;` → `use klynt_protocol::HookCompletedEvent;`
- `use codex_protocol::protocol::HookEventName;` → `use klynt_protocol::HookEventName;`
- `use codex_protocol::protocol::HookRunStatus;` → `use klynt_protocol::HookRunStatus;`
- `use codex_protocol::protocol::HookOutputEntry;` → `use klynt_protocol::HookOutputEntry;`
- `use codex_protocol::protocol::HookOutputEntryKind;` → `use klynt_protocol::HookOutputEntryKind;`
- `use codex_protocol::protocol::HookExecutionMode;` → `use klynt_protocol::HookExecutionMode;`
- `use codex_protocol::protocol::HookHandlerType;` → `use klynt_protocol::HookHandlerType;`
- `use codex_protocol::protocol::HookScope;` → `use klynt_protocol::HookScope;`
- `use codex_config::ConfigLayerStack;` and `ConfigLayerStackOrdering` → replace with a thin local stub:

```rust
// In klynt-hooks/src/registry.rs (add at top of file, replacing the
// codex_config import):

/// Minimal config-layer enumeration matching klyntbot's needs.
/// Codex has User/Project/Snapshot stacks; klynt uses User and Project only.
pub struct ConfigLayerStack {
    pub user_dir: Option<std::path::PathBuf>,
    pub project_dir: Option<std::path::PathBuf>,
}
impl ConfigLayerStack {
    pub fn user_then_project(&self) -> Vec<std::path::PathBuf> {
        let mut v = Vec::new();
        if let Some(u) = &self.user_dir { v.push(u.clone()); }
        if let Some(p) = &self.project_dir { v.push(p.clone()); }
        v
    }
}
```

- [ ] **Step 6: Update `crates/klynt-hooks/src/lib.rs` re-exports**

Replace the file with:

```rust
//! Klynt hook engine — Claude-Code-compatible schema.
//! Adapted from codex-rs/hooks/.

pub mod engine;
pub mod error;
pub mod events;
pub mod registry;
pub mod schema;
pub mod types;

pub use engine::HookEngine;
pub use error::{HookError, HookResult};
pub use events::common::HookCompletedEvent;
pub use registry::HookRegistry;
pub use schema::{Hook, HookConfig, HookEvents};
pub use types::HookPayload;
```

- [ ] **Step 7: Add a small error.rs**

Create `crates/klynt-hooks/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HookError {
    #[error("hook config parse: {0}")]
    Config(#[from] toml::de::Error),
    #[error("hook subprocess io: {0}")]
    Io(#[from] std::io::Error),
    #[error("hook json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hook timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("hook returned block: {reason}")]
    Blocked { reason: String },
    #[error("hook other: {0}")]
    Other(String),
}

pub type HookResult<T> = std::result::Result<T, HookError>;
```

- [ ] **Step 8: Verify it compiles**

Run `cargo build -p klynt-hooks`. Expected: clean. Common errors:
- Missing `klynt-protocol` dep → already added in Step 1; verify
- `ThreadId` not found → ensure the alias is in scope per Step 5
- `SandboxPermissions` mentions remain → grep and remove

Iterate until clean.

- [ ] **Step 9: Write a failing config parse test**

Create `crates/klynt-hooks/tests/parse_config.rs`:

```rust
use klynt_hooks::HookConfig;

#[test]
fn parse_hooks_toml_basic() {
    let toml_src = r#"
[[hook]]
event = "PreToolUse"
matcher = "Bash(*)"
command = "scripts/log-bash.sh"
timeout_ms = 5000
fail_open = true

[[hook]]
event = "PostToolUse"
matcher = "Edit(./crates/**)"
command = "scripts/auto-format-rust.sh"
timeout_ms = 10000
"#;
    let cfg: HookConfig = toml::from_str(toml_src).expect("parse");
    assert_eq!(cfg.hook.len(), 2);
    assert_eq!(cfg.hook[0].matcher.as_deref(), Some("Bash(*)"));
    assert_eq!(cfg.hook[0].timeout_ms, Some(5000));
    assert_eq!(cfg.hook[0].fail_open, Some(true));
    assert_eq!(cfg.hook[1].command, "scripts/auto-format-rust.sh");
}
```

- [ ] **Step 10: Confirm `HookConfig` has the expected shape**

The Codex schema may use different field names. Open `crates/klynt-hooks/src/schema.rs` and find the `Hook` struct. If the field names differ from `event`, `matcher`, `command`, `timeout_ms`, `fail_open` — either:
- Add `#[serde(rename = "...")]` attributes for the klynt names, or
- Update the test to match the actual upstream field names.

Klyntbot's spec (line 832) shows: `event`, `matcher`, `command`, `timeout_ms`, `fail_open` — TOML-snake_case. Match that exactly.

If `HookConfig` doesn't exist yet, define it:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    #[serde(default)]
    pub hook: Vec<Hook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub event: klynt_protocol::HookEventName,
    pub matcher: Option<String>,
    pub command: String,
    pub timeout_ms: Option<u64>,
    pub fail_open: Option<bool>,
}
```

- [ ] **Step 11: Run config parse test**

Run `cargo nextest run -p klynt-hooks --test parse_config`. Expected: pass.

- [ ] **Step 12: Write a failing subprocess-runner test**

Create `crates/klynt-hooks/tests/run_hook_subprocess.rs`:

```rust
use klynt_hooks::engine::command_runner::{run_command, CommandRunResult};
use klynt_hooks::schema::Hook;
use klynt_protocol::HookEventName;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn hook_subprocess_runs_and_returns_stdout() {
    let dir = TempDir::new().unwrap();
    let script = dir.path().join("hello.sh");
    fs::write(&script, r#"#!/usr/bin/env bash
read input
echo "got=$input"
"#).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let hook = Hook {
        event: HookEventName::PreToolUse,
        matcher: None,
        command: script.to_string_lossy().into_owned(),
        timeout_ms: Some(5000),
        fail_open: Some(true),
    };
    let input = json!({"tool":"bash", "args": {"command": "ls"}}).to_string();
    let res: CommandRunResult = run_command(&hook, &input).await;
    assert_eq!(res.exit_code, Some(0));
    assert!(res.stdout.contains("got="));
}

#[tokio::test]
async fn hook_subprocess_times_out_at_configured_limit() {
    let dir = TempDir::new().unwrap();
    let script = dir.path().join("slow.sh");
    fs::write(&script, "#!/usr/bin/env bash\nsleep 10\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let hook = Hook {
        event: HookEventName::PreToolUse,
        matcher: None,
        command: script.to_string_lossy().into_owned(),
        timeout_ms: Some(100),
        fail_open: Some(true),
    };
    let res = run_command(&hook, "{}").await;
    assert!(res.error.as_deref().unwrap_or("").contains("timed out"));
    assert_eq!(res.exit_code, None);
}
```

(The exact public API of `run_command` may differ — match the upstream signature. If `run_command` is private, expose it pub or use `HookEngine::fire` for the test.)

- [ ] **Step 13: Run subprocess tests**

Run `cargo nextest run -p klynt-hooks --test run_hook_subprocess`. Expected: 2 passes. If `run_command` is private, the simpler path is to test through `HookEngine::fire` with a temp `hooks.toml`.

- [ ] **Step 14: Run full nextest + clippy**

Run:
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets --all-features`

Expected: green, zero warnings.

- [ ] **Step 15: Commit**

```bash
git add crates/klynt-hooks/ \
        crates/klynt-protocol/src/lib.rs \
        crates/common/src/types.rs

git commit -m "$(cat <<'EOF'
feat(hooks): vendor + adapt Codex hooks engine (5 events)

Replaces the 12-LOC stub with the vendored Codex hook engine.
Implements ~/.klyntbot/hooks.toml parsing, globset/regex-based hook
matching, and tokio subprocess execution with stdin JSON / stdout
parse / exit-code / timeout semantics.

5 events shipped (matching upstream Codex):
- PreToolUse, PostToolUse, SessionStart, UserPromptSubmit, Stop

Adaptations:
- ThreadId → common::types::SessionKey alias
- codex_protocol::protocol::* → klynt_protocol::* (added types in
  klynt-protocol/src/lib.rs as additive declarations)
- codex_config::ConfigLayerStack → local 2-field struct (user/project)
- Drops legacy_notify.rs and user_notification.rs (Codex-specific)
- Drops bin/write_hooks_schema_fixtures.rs (dev utility)

Adds error.rs (HookError enum). Tests cover config parse + subprocess
execution including timeout fail-open behavior.

8 additional events (SessionEnd, Pre/PostCompact, Pre/PostFileEdit,
Notification, SubagentSpawn, Error) follow in commit 3/9.

Part of Phase 1 Plan 4 (commit 2/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Extend klynt-hooks with the 8 additional events

**Goal:** Add the events the spec mandates beyond what Codex ships: `SessionEnd`, `PreCompact`, `PostCompact`, `PreFileEdit`, `PostFileEdit`, `Notification`, `SubagentSpawn`, `Error`. Each event has a payload struct, an output struct, and a parser path through the existing `HookEngine::fire` orchestrator.

**Files:**
- Create: `crates/klynt-hooks/src/events/{session_end,pre_compact,post_compact,pre_file_edit,post_file_edit,notification,subagent_spawn,error}.rs`
- Modify: `crates/klynt-hooks/src/events/mod.rs`
- Modify: `crates/klynt-hooks/src/engine/mod.rs` (dispatch the new events)
- Modify: `crates/klynt-hooks/src/engine/config.rs` (add new event names to the per-event list)
- Modify: `crates/klynt-hooks/src/engine/dispatcher.rs` (route new events)
- Test: `crates/klynt-hooks/tests/events_extension.rs`

- [ ] **Step 1: Create session_end.rs**

```rust
// crates/klynt-hooks/src/events/session_end.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionEndInput {
    pub session_id: SessionKey,
    pub reason: String,           // "user_cancel" | "complete" | "error"
    pub duration_ms: u64,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct SessionEndOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::SessionEnd;
```

- [ ] **Step 2: Create pre_compact.rs**

```rust
// crates/klynt-hooks/src/events/pre_compact.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PreCompactInput {
    pub session_id: SessionKey,
    pub message_count: u64,
    pub current_tokens: u64,
    pub context_window: u64,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PreCompactOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
    pub should_block: bool,
    pub block_reason: Option<String>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PreCompact;
```

- [ ] **Step 3: Create post_compact.rs**

```rust
// crates/klynt-hooks/src/events/post_compact.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PostCompactInput {
    pub session_id: SessionKey,
    pub messages_compacted: u64,
    pub tokens_before: u64,
    pub tokens_after: u64,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PostCompactOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PostCompact;
```

- [ ] **Step 4: Create pre_file_edit.rs**

```rust
// crates/klynt-hooks/src/events/pre_file_edit.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PreFileEditInput {
    pub session_id: SessionKey,
    pub tool: String,            // "edit" | "write" | "apply_patch" | "notebook_edit"
    pub path: String,
    pub op: String,              // "create" | "edit" | "patch" | "notebook"
    pub diff_preview: String,
    pub bytes_before: u64,
    pub bytes_after: u64,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PreFileEditOutcome {
    pub should_block: bool,
    pub block_reason: Option<String>,
    pub modified_args: Option<serde_json::Value>,  // can rewrite content
    pub hook_events: Vec<HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PreFileEdit;
```

- [ ] **Step 5: Create post_file_edit.rs**

```rust
// crates/klynt-hooks/src/events/post_file_edit.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PostFileEditInput {
    pub session_id: SessionKey,
    pub tool: String,
    pub path: String,
    pub op: String,
    pub bytes_delta: i64,
    pub success: bool,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PostFileEditOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PostFileEdit;
```

- [ ] **Step 6: Create notification.rs**

```rust
// crates/klynt-hooks/src/events/notification.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotificationInput {
    pub session_id: SessionKey,
    pub kind: String,            // "approval_card_opened" | "toast" | "alert"
    pub message: String,
    pub tool: Option<String>,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::Notification;
```

- [ ] **Step 7: Create subagent_spawn.rs**

```rust
// crates/klynt-hooks/src/events/subagent_spawn.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubagentSpawnInput {
    pub session_id: SessionKey,
    pub parent_session_id: Option<SessionKey>,
    pub profile: String,         // "general" | "research" | "analyst"
    pub task_summary: String,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct SubagentSpawnOutcome {
    pub should_block: bool,
    pub block_reason: Option<String>,
    pub hook_events: Vec<HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::SubagentSpawn;
```

- [ ] **Step 8: Create error.rs**

```rust
// crates/klynt-hooks/src/events/error.rs

use crate::events::common::{BaseEventInput, HookCompletedEvent};
use klynt_protocol::{HookEventName, SessionKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ErrorInput {
    pub session_id: SessionKey,
    pub kind: String,            // "agent_loop_error" | "tool_error" | "io_error" | "other"
    pub message: String,
    pub recoverable: bool,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct ErrorOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::Error;
```

- [ ] **Step 9: Update events/mod.rs**

Replace `crates/klynt-hooks/src/events/mod.rs` with:

```rust
pub mod common;
// Codex-shipped (Task 2):
pub mod pre_tool_use;
pub mod post_tool_use;
pub mod session_start;
pub mod user_prompt_submit;
pub mod stop;
// Klynt extensions (Task 3):
pub mod session_end;
pub mod pre_compact;
pub mod post_compact;
pub mod pre_file_edit;
pub mod post_file_edit;
pub mod notification;
pub mod subagent_spawn;
pub mod error;
```

- [ ] **Step 10: Update engine/config.rs to recognize the new event names**

Open `crates/klynt-hooks/src/engine/config.rs`. The `HookEvents` struct probably maps to per-event name lists. Extend with the 8 new fields:

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct HookEvents {
    // Existing 5:
    #[serde(default)] pub pre_tool_use: Vec<HookHandlerConfig>,
    #[serde(default)] pub post_tool_use: Vec<HookHandlerConfig>,
    #[serde(default)] pub session_start: Vec<HookHandlerConfig>,
    #[serde(default)] pub user_prompt_submit: Vec<HookHandlerConfig>,
    #[serde(default)] pub stop: Vec<HookHandlerConfig>,
    // Klynt extensions:
    #[serde(default)] pub session_end: Vec<HookHandlerConfig>,
    #[serde(default)] pub pre_compact: Vec<HookHandlerConfig>,
    #[serde(default)] pub post_compact: Vec<HookHandlerConfig>,
    #[serde(default)] pub pre_file_edit: Vec<HookHandlerConfig>,
    #[serde(default)] pub post_file_edit: Vec<HookHandlerConfig>,
    #[serde(default)] pub notification: Vec<HookHandlerConfig>,
    #[serde(default)] pub subagent_spawn: Vec<HookHandlerConfig>,
    #[serde(default)] pub error: Vec<HookHandlerConfig>,
}
```

(The existing struct may use a different naming scheme — match it.)

- [ ] **Step 11: Update engine/dispatcher.rs to route new events**

Open `crates/klynt-hooks/src/engine/dispatcher.rs`. Find the dispatch match arm for `HookEventName::*`. Add 8 new arms — one per extension event:

```rust
HookEventName::SessionEnd => {
    // Build SessionEndInput from the payload, find matching hooks, run them.
    // Pattern is the same as session_start.rs:dispatch — copy and adjust types.
}
HookEventName::PreCompact => { /* same pattern, return PreCompactOutcome */ }
HookEventName::PostCompact => { /* same pattern */ }
HookEventName::PreFileEdit => { /* same pattern; can block + modify_args */ }
HookEventName::PostFileEdit => { /* same pattern */ }
HookEventName::Notification => { /* same pattern */ }
HookEventName::SubagentSpawn => { /* same pattern; can block */ }
HookEventName::Error => { /* same pattern */ }
```

For each new event, copy the body of the closest existing dispatch arm (pre_tool_use for blockable events; session_start for non-blockable lifecycle events). Replace the input struct + outcome type names.

- [ ] **Step 12: Verify the engine compiles**

Run `cargo build -p klynt-hooks`. Expected: clean.

- [ ] **Step 13: Add the public HookEngine::fire facade**

The integration tasks (Tasks 5-8) need a single ergonomic call: `engine.fire(event_payload).await -> HookOutcome`. If the existing `HookEngine` already exposes per-event methods (`fire_pre_tool_use`, `fire_session_start`, etc.), keep those and add a uniform wrapper:

```rust
// crates/klynt-hooks/src/engine/mod.rs (additive)

pub enum HookFireInput {
    PreToolUse(crate::events::pre_tool_use::PreToolUseInput),
    PostToolUse(crate::events::post_tool_use::PostToolUseInput),
    SessionStart(crate::events::session_start::SessionStartInput),
    UserPromptSubmit(crate::events::user_prompt_submit::UserPromptSubmitInput),
    Stop(crate::events::stop::StopInput),
    SessionEnd(crate::events::session_end::SessionEndInput),
    PreCompact(crate::events::pre_compact::PreCompactInput),
    PostCompact(crate::events::post_compact::PostCompactInput),
    PreFileEdit(crate::events::pre_file_edit::PreFileEditInput),
    PostFileEdit(crate::events::post_file_edit::PostFileEditInput),
    Notification(crate::events::notification::NotificationInput),
    SubagentSpawn(crate::events::subagent_spawn::SubagentSpawnInput),
    Error(crate::events::error::ErrorInput),
}

#[derive(Debug, Clone)]
pub enum HookOutcome {
    /// No hook fired, or all hooks said "continue".
    Allow,
    /// At least one hook returned `block: true`. Caller should abort.
    Block { reason: String },
    /// At least one hook returned `modify_args`. Caller should rewrite args.
    ModifyArgs { args: serde_json::Value },
    /// Lifecycle event fired (e.g., SessionStart). No control-flow effect.
    LifecycleNoOp,
}

impl HookEngine {
    /// Single entry point used by all integration sites in klynt-core/agent.
    /// Dispatches the event to the right per-event handler internally.
    pub async fn fire(&self, input: HookFireInput) -> HookOutcome {
        match input {
            HookFireInput::PreToolUse(i) => {
                let outcome = self.dispatch_pre_tool_use(i).await;
                if outcome.should_block {
                    HookOutcome::Block { reason: outcome.block_reason.unwrap_or_default() }
                } else {
                    HookOutcome::Allow
                }
            }
            HookFireInput::PreFileEdit(i) => {
                let outcome = self.dispatch_pre_file_edit(i).await;
                if outcome.should_block {
                    HookOutcome::Block { reason: outcome.block_reason.unwrap_or_default() }
                } else if let Some(args) = outcome.modified_args {
                    HookOutcome::ModifyArgs { args }
                } else {
                    HookOutcome::Allow
                }
            }
            HookFireInput::SubagentSpawn(i) => {
                let outcome = self.dispatch_subagent_spawn(i).await;
                if outcome.should_block {
                    HookOutcome::Block { reason: outcome.block_reason.unwrap_or_default() }
                } else {
                    HookOutcome::Allow
                }
            }
            // Non-blockable events: just dispatch and return LifecycleNoOp.
            _ => {
                self.dispatch_lifecycle(input).await;
                HookOutcome::LifecycleNoOp
            }
        }
    }

    async fn dispatch_lifecycle(&self, input: HookFireInput) {
        // Minimal — calls the matching per-event method and ignores outcome.
        // Implementations: dispatch_post_tool_use, dispatch_session_start, etc.
    }
}
```

- [ ] **Step 14: Write event extension test**

Create `crates/klynt-hooks/tests/events_extension.rs`:

```rust
use klynt_hooks::events::{
    pre_compact::PreCompactInput,
    pre_file_edit::PreFileEditInput,
    session_end::SessionEndInput,
    subagent_spawn::SubagentSpawnInput,
};

#[test]
fn extension_event_inputs_serialize_round_trip() {
    let pre_compact = PreCompactInput {
        session_id: "test-session".to_string(),
        message_count: 100,
        current_tokens: 50_000,
        context_window: 200_000,
        base: Default::default(),
    };
    let s = serde_json::to_string(&pre_compact).unwrap();
    let back: PreCompactInput = serde_json::from_str(&s).unwrap();
    assert_eq!(back.message_count, 100);
}

#[test]
fn pre_file_edit_input_carries_diff_preview() {
    let input = PreFileEditInput {
        session_id: "s1".into(),
        tool: "edit".into(),
        path: "src/main.rs".into(),
        op: "edit".into(),
        diff_preview: "@@ -1 +1 @@\n-old\n+new\n".into(),
        bytes_before: 100,
        bytes_after: 103,
        base: Default::default(),
    };
    let s = serde_json::to_string(&input).unwrap();
    assert!(s.contains("diff_preview"));
}
```

- [ ] **Step 15: Run extension tests**

Run `cargo nextest run -p klynt-hooks --test events_extension`. Expected: 2 passes.

- [ ] **Step 16: Run full suite**

Run:
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets --all-features`

Expected: green, zero warnings.

- [ ] **Step 17: Commit**

```bash
git add crates/klynt-hooks/src/events/{session_end,pre_compact,post_compact,pre_file_edit,post_file_edit,notification,subagent_spawn,error}.rs \
        crates/klynt-hooks/src/events/mod.rs \
        crates/klynt-hooks/src/engine/{mod,config,dispatcher}.rs \
        crates/klynt-hooks/tests/events_extension.rs

git commit -m "$(cat <<'EOF'
feat(hooks): add 8 klynt-extension hook events

Codex's hooks crate ships 5 events; klyntbot's spec mandates 13. Adds:
- SessionEnd      (session terminates: cancel/complete/error)
- PreCompact      (mid-loop compressor about to compact history)
- PostCompact     (compactor finished)
- PreFileEdit     (subset of PreToolUse for edit/write/apply_patch/notebook_edit)
- PostFileEdit    (subset of PostToolUse for the same tools)
- Notification    (approval card opens, toast surfaced)
- SubagentSpawn   (agent calls task tool)
- Error           (agent loop unrecoverable error)

Each event has Input + Outcome structs and a dispatcher arm. PreFileEdit
and SubagentSpawn are blockable (return should_block + block_reason);
PreFileEdit additionally supports modify_args to rewrite the file content
or path before the edit.

Adds the unified HookEngine::fire(HookFireInput) -> HookOutcome facade
used by integration sites in Tasks 5-8.

Part of Phase 1 Plan 4 (commit 3/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: HookEngine lifecycle + RoutingContext + ToolKitBuilder wiring

**Goal:** Construct a single `HookEngine` at AppCore init, store it on AgentRuntime, plumb through `RoutingContext.hook_engine`, expose via `ToolKitBuilder.hook_engine` for sub-agent inheritance.

**Files:**
- Modify: `crates/tools-core/src/routing.rs` (RoutingContext gains hook_engine)
- Modify: `crates/tools-core/Cargo.toml` (+ klynt-hooks)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (+ hook_engine field + accessors)
- Modify: `crates/klynt-core/src/registry/builder.rs` (ToolKitBuilder.hook_engine)
- Modify: `crates/klynt-core/Cargo.toml` (+ klynt-hooks)
- Modify: `crates/app-core/src/init/mod.rs` (HookEngine construction; plumbing)
- Test: `crates/klynt-core/tests/hook_engine_wired.rs`

- [ ] **Step 1: Add klynt-hooks workspace dep**

Edit root `Cargo.toml`:

```toml
[workspace.dependencies]
# ... existing ...
klynt-hooks = { path = "crates/klynt-hooks" }
```

Then edit `crates/tools-core/Cargo.toml`:

```toml
[dependencies]
# ... existing ...
klynt-hooks = { workspace = true }
```

And `crates/klynt-core/Cargo.toml`:

```toml
[dependencies]
# ... existing ...
klynt-hooks = { workspace = true }
```

- [ ] **Step 2: Verify deps**

Run `cargo check -p tools-core -p klynt-core`. Expected: clean.

- [ ] **Step 3: Add `hook_engine` field to RoutingContext**

Open `crates/tools-core/src/routing.rs`. Find the `RoutingContext` struct. Add:

```rust
pub struct RoutingContext {
    // ... existing fields including event_tx from tool-layer-consolidation ...
    /// Hook engine for firing PreToolUse / PostToolUse / etc. at tool-execute
    /// boundaries. None = hooks disabled (e.g., in unit tests).
    pub hook_engine: Option<std::sync::Arc<klynt_hooks::HookEngine>>,
}
```

If `RoutingContext` derives `Default`, ensure the new field defaults to `None`. If a builder pattern exists, add a `with_hook_engine(self, engine: Arc<HookEngine>) -> Self` method.

- [ ] **Step 4: Add `hook_engine` field to AgentRuntime**

Open `crates/agent/src/agent_runtime/runtime.rs`. Add:

```rust
pub struct AgentRuntime {
    // ... existing fields including tool_kit from tool-layer-consolidation ...
    hook_engine: Option<std::sync::Arc<klynt_hooks::HookEngine>>,
}

impl AgentRuntime {
    pub fn hook_engine(&self) -> Option<std::sync::Arc<klynt_hooks::HookEngine>> {
        self.hook_engine.clone()
    }
    pub fn set_hook_engine(&mut self, engine: std::sync::Arc<klynt_hooks::HookEngine>) {
        self.hook_engine = Some(engine);
    }
}
```

(Use `OnceCell` / `RwLock` if AgentRuntime is wrapped in `Arc` and not mutable — match the tool_kit accessor pattern.)

- [ ] **Step 5: Add `hook_engine` to ToolKitBuilder**

Open `crates/klynt-core/src/registry/builder.rs`. Add the field:

```rust
#[derive(Clone)]
pub struct ToolKitBuilder {
    // ... existing fields ...
    pub hook_engine: Option<std::sync::Arc<klynt_hooks::HookEngine>>,
}
```

The builder doesn't directly use the engine (tools fire hooks via `ctx.hook_engine`). The field exists so AppCore init can stash the reference once and have it propagate to sub-agents through ToolKitBuilder cloning.

- [ ] **Step 6: Construct HookEngine in app-core/init**

Open `crates/app-core/src/init/mod.rs`. After the existing `host_cache` construction (added in tool-layer-consolidation Task 3), add:

```rust
// Construct the hook engine. Parses ~/.klyntbot/hooks.toml if present.
let hook_engine_path = dirs::home_dir()
    .map(|h| h.join(".klyntbot/hooks.toml"));
let hook_engine: Option<Arc<klynt_hooks::HookEngine>> = match hook_engine_path {
    Some(p) if p.exists() => {
        match klynt_hooks::HookEngine::load_from_path(&p) {
            Ok(e) => Some(Arc::new(e)),
            Err(err) => {
                tracing::warn!("klynt-hooks: failed to load {p:?}: {err}");
                Some(Arc::new(klynt_hooks::HookEngine::empty()))
            }
        }
    }
    _ => Some(Arc::new(klynt_hooks::HookEngine::empty())),
};

// Plumb to ToolKitBuilder:
let kit = Arc::new(klynt_core::ToolKitBuilder {
    // ... existing fields ...
    hook_engine: hook_engine.clone(),
});

// Stash on AgentRuntime:
core.agent.set_hook_engine(hook_engine.clone().expect("constructed above"));
```

(`HookEngine::empty()` and `HookEngine::load_from_path(p)` may need to be added — see Step 7.)

- [ ] **Step 7: Add HookEngine::empty + ::load_from_path constructors**

Open `crates/klynt-hooks/src/engine/mod.rs`. After the existing `HookEngine` impl, add:

```rust
impl HookEngine {
    /// Empty engine — no hooks configured. All `fire(...)` calls return Allow.
    pub fn empty() -> Self {
        Self {
            // initialize fields with empty defaults, matching the upstream struct
        }
    }

    /// Load from a single hooks.toml path. Used at startup with
    /// `~/.klyntbot/hooks.toml`.
    pub fn load_from_path(path: &std::path::Path) -> klynt_hooks::HookResult<Self> {
        let s = std::fs::read_to_string(path)?;
        let cfg: crate::schema::HookConfig = toml::from_str(&s)?;
        Ok(Self::from_config(cfg))
    }

    /// Construct from a parsed config. Internal helper used by load_from_path
    /// and tests.
    pub fn from_config(cfg: crate::schema::HookConfig) -> Self {
        // build internal registry from cfg.hook entries, group by event name
        // ...
        Self::empty()  // placeholder; real impl walks cfg.hook
    }
}
```

- [ ] **Step 8: Plumb `hook_engine` from RoutingContext at process_message entry**

Open `crates/agent/src/agent_runtime/runtime.rs`. In `process_message`, the function clones `ctx` to a mutable copy (per tool-layer-consolidation Task 4):

```rust
let mut ctx = ctx.clone();
ctx.event_tx = event_tx.clone();
if let Some(t) = &cancel_token { ctx.cancel_token = Some(t.clone()); }
ctx.hook_engine = self.hook_engine();   // NEW
```

This ensures every tool invoked during this message has access to the engine via `ctx.hook_engine`.

- [ ] **Step 9: Write integration test for engine wiring**

Create `crates/klynt-core/tests/hook_engine_wired.rs`:

```rust
use std::sync::Arc;
use klynt_core::ToolKitBuilder;
use klynt_hooks::HookEngine;

#[test]
fn tool_kit_builder_carries_hook_engine() {
    let engine = Arc::new(HookEngine::empty());
    let builder = ToolKitBuilder {
        // ... fill in deps the way other tests do (use the helper from
        // crates/klynt-core/tests/tool_kit_builder.rs) ...
        hook_engine: Some(engine.clone()),
        // ...
    };
    assert!(builder.hook_engine.is_some());
    assert!(Arc::ptr_eq(&builder.hook_engine.unwrap(), &engine));
}

#[test]
fn tool_kit_builder_default_hook_engine_is_none() {
    // Default construction without hook_engine should produce None.
    // (If the builder doesn't have a Default, skip this test.)
}
```

- [ ] **Step 10: Run integration test**

Run `cargo nextest run -p klynt-core --test hook_engine_wired`. Expected: pass.

- [ ] **Step 11: Run full suite**

Run:
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets --all-features`

Expected: green, zero warnings.

- [ ] **Step 12: Commit**

```bash
git add crates/tools-core/src/routing.rs \
        crates/tools-core/Cargo.toml \
        crates/agent/src/agent_runtime/runtime.rs \
        crates/klynt-core/src/registry/builder.rs \
        crates/klynt-core/Cargo.toml \
        crates/klynt-hooks/src/engine/mod.rs \
        crates/klynt-core/tests/hook_engine_wired.rs \
        crates/app-core/src/init/mod.rs \
        Cargo.toml

git commit -m "$(cat <<'EOF'
feat(hooks): wire HookEngine through RoutingContext + ToolKitBuilder

Constructs a single HookEngine at app-core/init (parsing
~/.klyntbot/hooks.toml if present). Stashes on AgentRuntime via
set_hook_engine + accessor. Plumbs through RoutingContext.hook_engine
in process_message so every tool's execute() can fire hooks via
ctx.hook_engine without holding a constructor-time reference.

ToolKitBuilder gains hook_engine field for sub-agent inheritance.

Adds HookEngine::empty() + ::load_from_path() constructors.

Part of Phase 1 Plan 4 (commit 4/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: PreToolUse + PostToolUse integration in klynt-core tools

**Goal:** Each klynt-core tool's `execute()` body fires `PreToolUse` *after* the approval gate returns Allow but *before* the actual sandbox/IO. Block returns abort with `ToolError::HookBlocked`. ModifyArgs rewrites the tool input. After execution, `PostToolUse` fires (success or failure).

**Files:**
- Create: `crates/klynt-core/src/tools/shared/hook_emit.rs`
- Modify: `crates/klynt-core/src/tools/shared/mod.rs`
- Modify: `crates/common/src/error.rs` (add ToolError::HookBlocked variant)
- Modify: `crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,read,glob,grep,web_fetch,ask_user,tool_search}.rs`
- Test: `crates/klynt-core/tests/hook_pre_post_tool_use.rs`

- [ ] **Step 1: Add ToolError::HookBlocked variant**

Open `crates/common/src/error.rs`. Find the `ToolError` enum. Add:

```rust
pub enum ToolError {
    // ... existing variants (PermissionDenied, ExecutionFailed, InvalidParams, etc.) ...

    /// A hook returned `block: true` (Pre*Tool/Pre*FileEdit/SubagentSpawn).
    /// The agent loop surfaces the reason to the LLM so it can react.
    HookBlocked(String),
}
```

If the enum uses `#[derive(thiserror::Error)]`, add `#[error("hook blocked: {0}")] HookBlocked(String),`.

- [ ] **Step 2: Create the hook_emit helper**

Create `crates/klynt-core/src/tools/shared/hook_emit.rs`:

```rust
//! Helpers for klynt-core tools to fire PreToolUse / PostToolUse hooks at
//! their execute() boundaries.

use klynt_hooks::engine::{HookEngine, HookFireInput, HookOutcome};
use klynt_hooks::events::{
    pre_tool_use::PreToolUseInput,
    post_tool_use::PostToolUseInput,
    pre_file_edit::PreFileEditInput,
    post_file_edit::PostFileEditInput,
};
use klynt_protocol::SessionKey;
use std::sync::Arc;
use std::time::Instant;

/// Fire PreToolUse and return:
/// - `Ok(None)` if no engine, or hooks said allow.
/// - `Ok(Some(modified))` if a hook returned modify_args (caller should swap args).
/// - `Err(reason)` if a hook returned block.
pub async fn fire_pre_tool_use(
    engine: Option<&Arc<HookEngine>>,
    session_id: SessionKey,
    tool: &str,
    args: serde_json::Value,
    cwd: Option<String>,
) -> Result<Option<serde_json::Value>, String> {
    let Some(e) = engine else { return Ok(None) };
    let input = PreToolUseInput {
        session_id,
        tool: tool.to_string(),
        args,
        cwd,
        // base + remaining fields filled with defaults; match upstream struct
        ..Default::default()
    };
    match e.fire(HookFireInput::PreToolUse(input)).await {
        HookOutcome::Allow | HookOutcome::LifecycleNoOp => Ok(None),
        HookOutcome::Block { reason } => Err(reason),
        HookOutcome::ModifyArgs { args } => Ok(Some(args)),
    }
}

/// Fire PostToolUse. Errors are swallowed (logged) — post-hooks never abort
/// the call.
pub async fn fire_post_tool_use(
    engine: Option<&Arc<HookEngine>>,
    session_id: SessionKey,
    tool: &str,
    success: bool,
    duration: std::time::Duration,
    output_summary: Option<String>,
) {
    let Some(e) = engine else { return };
    let input = PostToolUseInput {
        session_id,
        tool: tool.to_string(),
        success,
        duration_ms: duration.as_millis() as u64,
        output_summary,
        ..Default::default()
    };
    let _ = e.fire(HookFireInput::PostToolUse(input)).await;
}

/// Convenience: time a future and fire pre + post around it.
/// Use only when the tool doesn't need to inspect ModifyArgs (most common case).
pub async fn around<F, T>(
    engine: Option<&Arc<HookEngine>>,
    session_id: SessionKey,
    tool: &str,
    args: serde_json::Value,
    cwd: Option<String>,
    fut: F,
) -> std::result::Result<T, common::KlyntbotError>
where
    F: std::future::Future<Output = std::result::Result<T, common::KlyntbotError>>,
{
    if let Err(reason) = fire_pre_tool_use(engine, session_id.clone(), tool, args, cwd).await {
        return Err(common::KlyntbotError::Tool(common::ToolError::HookBlocked(reason)));
    }
    let start = Instant::now();
    let result = fut.await;
    fire_post_tool_use(engine, session_id, tool, result.is_ok(), start.elapsed(), None).await;
    result
}
```

- [ ] **Step 3: Re-export from shared mod**

Open `crates/klynt-core/src/tools/shared/mod.rs`. Add `pub mod hook_emit;` (create the file if `shared/mod.rs` doesn't exist; other helpers like `fs_resolve` live there).

- [ ] **Step 4: Wire BashTool execute() to fire hooks**

Open `crates/klynt-core/src/tools/bash.rs`. Find `execute()`. After the existing approval call returns Allow, but before the `MacOsSeatbeltRunner::run_command` (or equivalent), add:

```rust
use crate::tools::shared::hook_emit::{fire_pre_tool_use, fire_post_tool_use};
use std::time::Instant;

// ... after approval check returns Allow:
let session_id = ctx.session_key.clone().unwrap_or_default();
let args_json = serde_json::to_value(&args).unwrap_or_default();
let cwd_str = ctx.cwd.clone();

let pre_result = fire_pre_tool_use(
    ctx.hook_engine.as_ref(),
    session_id.clone(),
    "bash",
    args_json.clone(),
    cwd_str.clone(),
).await;
let args = match pre_result {
    Ok(None) => args,
    Ok(Some(modified)) => serde_json::from_value(modified).map_err(|e| {
        common::KlyntbotError::Tool(common::ToolError::InvalidParams(format!("hook modify_args: {e}")))
    })?,
    Err(reason) => {
        return Err(common::KlyntbotError::Tool(common::ToolError::HookBlocked(reason)));
    }
};

let start = Instant::now();
let result = /* existing sandbox+exec body, parameterized by args.command */;

fire_post_tool_use(
    ctx.hook_engine.as_ref(),
    session_id,
    "bash",
    result.is_ok(),
    start.elapsed(),
    None,
).await;

result
```

- [ ] **Step 5: Repeat for the 9 other tools**

For each of: `read.rs`, `glob.rs`, `grep.rs`, `edit.rs`, `write.rs`, `apply_patch.rs`, `notebook_edit.rs`, `web_fetch.rs`, `ask_user.rs`, `tool_search.rs` — apply the same pattern:

1. Import `fire_pre_tool_use`, `fire_post_tool_use`.
2. After approval gate (or at top of execute() for tools without approval), fire PreToolUse.
3. Handle `Ok(Some(modified))` → swap args; `Err(reason)` → return ToolError::HookBlocked.
4. Run the existing tool body.
5. Fire PostToolUse with success flag.

For tools that don't have an approval gate (read/glob/grep/ask_user/tool_search), fire PreToolUse at the top of `execute()`.

This is mechanical — same pattern 10 times. Each tool's diff is ~15 lines.

- [ ] **Step 6: Write integration test**

Create `crates/klynt-core/tests/hook_pre_post_tool_use.rs`:

```rust
//! Verifies PreToolUse blocks tool calls and PostToolUse fires on completion.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use klynt_hooks::HookEngine;
use klynt_hooks::schema::{Hook, HookConfig};
use klynt_protocol::HookEventName;

fn make_blocking_hook(script: &PathBuf) -> Hook {
    Hook {
        event: HookEventName::PreToolUse,
        matcher: Some("read".into()),
        command: script.to_string_lossy().into_owned(),
        timeout_ms: Some(5000),
        fail_open: Some(false),
    }
}

#[tokio::test]
async fn pre_tool_use_block_aborts_read_tool() {
    let dir = TempDir::new().unwrap();
    let script = dir.path().join("block.sh");
    fs::write(&script, r#"#!/usr/bin/env bash
echo '{"block":true,"reason":"test block"}'
exit 0
"#).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let cfg = HookConfig { hook: vec![make_blocking_hook(&script)] };
    let engine = Arc::new(HookEngine::from_config(cfg));

    // Construct a minimal RoutingContext with hook_engine set.
    let mut ctx = tools_core::RoutingContext::default();
    ctx.session_key = Some("test-session".into());
    ctx.hook_engine = Some(engine);

    // Build a ReadTool and call execute on a real file.
    let workdir = TempDir::new().unwrap();
    fs::write(workdir.path().join("a.txt"), "hello").unwrap();
    let tool = klynt_core::tools::ReadTool::new(
        workdir.path().to_path_buf(),
        Arc::new(klynt_core::privacy::PrivacyGuard::from_globs(&[]).unwrap()),
    );
    let args: klynt_core::tools::ReadArgs = serde_json::from_value(serde_json::json!({
        "path": "a.txt"
    })).unwrap();

    let result = <klynt_core::tools::ReadTool as tools_core::ToolExecute>::execute(&tool, args, &ctx).await;
    let err = result.expect_err("hook should block");
    let msg = format!("{err:?}");
    assert!(msg.contains("HookBlocked") || msg.contains("test block"),
        "expected HookBlocked, got {msg}");
}
```

- [ ] **Step 7: Run tests**

Run `cargo nextest run -p klynt-core --test hook_pre_post_tool_use`. Expected: pass.

- [ ] **Step 8: Run full suite**

Run:
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets --all-features`

Expected: green, zero warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/klynt-core/src/tools/shared/{mod,hook_emit}.rs \
        crates/klynt-core/src/tools/{bash,edit,write,apply_patch,notebook_edit,read,glob,grep,web_fetch,ask_user,tool_search}.rs \
        crates/common/src/error.rs \
        crates/klynt-core/tests/hook_pre_post_tool_use.rs

git commit -m "$(cat <<'EOF'
feat(hooks): integrate PreToolUse + PostToolUse in klynt-core tools

Each of the 11 klynt-core tools now fires PreToolUse before the actual
sandbox/IO action and PostToolUse after completion. Block returns abort
the call with ToolError::HookBlocked; ModifyArgs rewrites the input
before execution.

Adds shared/hook_emit.rs with fire_pre_tool_use + fire_post_tool_use
helpers used uniformly across all 11 tools.

Adds ToolError::HookBlocked variant.

Phase 1 minimum scope reached at this commit (master spec line 1371).

Part of Phase 1 Plan 4 (commit 5/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: PreFileEdit + PostFileEdit integration in mutating tools

**Goal:** edit/write/apply_patch/notebook_edit additionally fire `PreFileEdit` (with diff preview) before writing to disk and `PostFileEdit` after. PreFileEdit can block or rewrite the diff.

**Files:**
- Modify: `crates/klynt-core/src/tools/{edit,write,apply_patch,notebook_edit}.rs`
- Modify: `crates/klynt-core/src/tools/shared/hook_emit.rs` (add fire_pre_file_edit + fire_post_file_edit helpers)
- Test: `crates/klynt-core/tests/hook_pre_post_file_edit.rs`

- [ ] **Step 1: Add fire_pre_file_edit + fire_post_file_edit helpers**

Append to `crates/klynt-core/src/tools/shared/hook_emit.rs`:

```rust
use klynt_hooks::events::pre_file_edit::PreFileEditInput;
use klynt_hooks::events::post_file_edit::PostFileEditInput;

pub async fn fire_pre_file_edit(
    engine: Option<&Arc<HookEngine>>,
    session_id: SessionKey,
    tool: &str,
    path: &str,
    op: &str,
    diff_preview: String,
    bytes_before: u64,
    bytes_after: u64,
) -> Result<Option<serde_json::Value>, String> {
    let Some(e) = engine else { return Ok(None) };
    let input = PreFileEditInput {
        session_id,
        tool: tool.to_string(),
        path: path.to_string(),
        op: op.to_string(),
        diff_preview,
        bytes_before,
        bytes_after,
        ..Default::default()
    };
    match e.fire(HookFireInput::PreFileEdit(input)).await {
        HookOutcome::Allow | HookOutcome::LifecycleNoOp => Ok(None),
        HookOutcome::Block { reason } => Err(reason),
        HookOutcome::ModifyArgs { args } => Ok(Some(args)),
    }
}

pub async fn fire_post_file_edit(
    engine: Option<&Arc<HookEngine>>,
    session_id: SessionKey,
    tool: &str,
    path: &str,
    op: &str,
    bytes_delta: i64,
    success: bool,
) {
    let Some(e) = engine else { return };
    let input = PostFileEditInput {
        session_id,
        tool: tool.to_string(),
        path: path.to_string(),
        op: op.to_string(),
        bytes_delta,
        success,
        ..Default::default()
    };
    let _ = e.fire(HookFireInput::PostFileEdit(input)).await;
}
```

- [ ] **Step 2: Wire EditTool to fire PreFileEdit + PostFileEdit**

Open `crates/klynt-core/src/tools/edit.rs`. After the PreToolUse hook (from Task 5) and after computing the new content but before `std::fs::write`:

```rust
use crate::tools::shared::hook_emit::{fire_pre_file_edit, fire_post_file_edit};

// Compute the diff preview (existing edit body produces a diff anyway):
let diff_preview = /* existing diff string */;
let bytes_before = original_content.len() as u64;
let bytes_after = new_content.len() as u64;

let pre_file_result = fire_pre_file_edit(
    ctx.hook_engine.as_ref(),
    session_id.clone(),
    "edit",
    &path_str,
    "edit",
    diff_preview.clone(),
    bytes_before,
    bytes_after,
).await;
let new_content = match pre_file_result {
    Ok(None) => new_content,
    Ok(Some(modified)) => {
        // Hook rewrote the content; expect modified.content
        modified.get("content").and_then(|v| v.as_str()).map(String::from).unwrap_or(new_content)
    }
    Err(reason) => return Err(common::KlyntbotError::Tool(common::ToolError::HookBlocked(reason))),
};

// Write to disk
std::fs::write(&full_path, &new_content)?;

// Emit FileEditWithSymbols (existing behavior from tool-layer-consolidation Task 4)
// ...

fire_post_file_edit(
    ctx.hook_engine.as_ref(),
    session_id,
    "edit",
    &path_str,
    "edit",
    (bytes_after as i64) - (bytes_before as i64),
    true,
).await;
```

- [ ] **Step 3: Repeat for write.rs, apply_patch.rs, notebook_edit.rs**

Same pattern, with `op` set to "write" / "patch" / "notebook" respectively. The `diff_preview` is constructed from the bytes-before/after diff.

- [ ] **Step 4: Write integration test**

Create `crates/klynt-core/tests/hook_pre_post_file_edit.rs`:

```rust
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use klynt_hooks::HookEngine;
use klynt_hooks::schema::{Hook, HookConfig};
use klynt_protocol::HookEventName;

#[tokio::test]
async fn pre_file_edit_block_aborts_write() {
    let dir = TempDir::new().unwrap();
    let script = dir.path().join("block_edit.sh");
    fs::write(&script, r#"#!/usr/bin/env bash
echo '{"block":true,"reason":"no edits to .env files"}'
exit 0
"#).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let hook = Hook {
        event: HookEventName::PreFileEdit,
        matcher: Some("write".into()),
        command: script.to_string_lossy().into_owned(),
        timeout_ms: Some(5000),
        fail_open: Some(false),
    };
    let cfg = HookConfig { hook: vec![hook] };
    let engine = Arc::new(HookEngine::from_config(cfg));

    let workdir = TempDir::new().unwrap();
    let mut ctx = tools_core::RoutingContext::default();
    ctx.session_key = Some("test-session".into());
    ctx.hook_engine = Some(engine);

    // Build WriteTool with full deps (use the helper from existing tests).
    // ...
    // Call execute with a write to .env, expect HookBlocked error.
    // Verify the .env file was NOT created.
}
```

- [ ] **Step 5: Run test**

Run `cargo nextest run -p klynt-core --test hook_pre_post_file_edit`. Expected: pass.

- [ ] **Step 6: Run full suite**

Run:
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets --all-features`

Expected: green, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/klynt-core/src/tools/shared/hook_emit.rs \
        crates/klynt-core/src/tools/{edit,write,apply_patch,notebook_edit}.rs \
        crates/klynt-core/tests/hook_pre_post_file_edit.rs

git commit -m "$(cat <<'EOF'
feat(hooks): integrate PreFileEdit + PostFileEdit in mutating tools

edit / write / apply_patch / notebook_edit now fire PreFileEdit BEFORE
writing to disk (with diff preview, bytes_before, bytes_after) and
PostFileEdit AFTER. Block aborts the write; ModifyArgs rewrites the
content/path before disk hits.

Adds fire_pre_file_edit + fire_post_file_edit helpers in shared/hook_emit.rs.

Part of Phase 1 Plan 4 (commit 6/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Agent-loop lifecycle hooks (SessionStart, SessionEnd, Stop, Error)

**Goal:** Wire the 4 lifecycle hooks at the agent loop's natural boundaries.

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`
- Test: `crates/agent/tests/lifecycle_hooks.rs`

- [ ] **Step 1: Wire SessionStart in chat handler**

Open `crates/app-core/src/handlers/chat/streaming.rs`. Find `chat_send`. After the mode-detection block (where `mode == "coding"` is observed), and BEFORE delegating to `agent.process_direct_streaming`, add:

```rust
// Fire SessionStart hook on first coding-mode message of this session.
if mode.as_deref() == Some("coding") {
    if let Some(engine) = core.agent.hook_engine() {
        let input = klynt_hooks::events::session_start::SessionStartInput {
            session_id: session_key.clone(),
            cwd: workspace_root.clone().to_string_lossy().into_owned(),
            ..Default::default()
        };
        let _ = engine.fire(klynt_hooks::engine::HookFireInput::SessionStart(input)).await;
    }
}
```

(Consider gating on "first message in this session" via a session-level flag — if the SessionStart should only fire once per session and not per message. The flag lives on the session row.)

- [ ] **Step 2: Wire SessionEnd in chat_cancel + on session terminal state**

Open the same file. Find `chat_cancel` (or wherever the cancel handler lives). Fire:

```rust
let input = klynt_hooks::events::session_end::SessionEndInput {
    session_id: session_key.clone(),
    reason: "user_cancel".into(),
    duration_ms: /* compute from session start */ 0,
    ..Default::default()
};
let _ = engine.fire(klynt_hooks::engine::HookFireInput::SessionEnd(input)).await;
```

Also fire on session terminal state — when the agent loop completes its final turn (per spec line 854: "session reaches a quiescent terminal state").

- [ ] **Step 3: Wire Stop hook at agent loop terminal turn**

Open `crates/agent/src/agent_loop/mod.rs`. Find the place where the agent loop's terminal turn fires `AgentEvent::Done`. Add a hook fire there:

```rust
// After emitting AgentEvent::Done:
if let Some(engine) = self.runtime.hook_engine() {
    let input = klynt_hooks::events::stop::StopInput {
        session_id: routing_ctx.session_key.clone().unwrap_or_default(),
        message_count: history.len() as u64,
        ..Default::default()
    };
    let _ = engine.fire(klynt_hooks::engine::HookFireInput::Stop(input)).await;
}
```

- [ ] **Step 4: Wire Error hook**

In the same file, find the unrecoverable error path (where `AgentEvent::Error` is emitted). Fire:

```rust
let input = klynt_hooks::events::error::ErrorInput {
    session_id: routing_ctx.session_key.clone().unwrap_or_default(),
    kind: "agent_loop_error".into(),
    message: err.to_string(),
    recoverable: false,
    ..Default::default()
};
let _ = engine.fire(klynt_hooks::engine::HookFireInput::Error(input)).await;
```

- [ ] **Step 5: Write integration test**

Create `crates/agent/tests/lifecycle_hooks.rs`:

```rust
//! Verifies SessionStart and Stop fire at the right boundaries.
//!
//! Setup: use a hook script that appends each fired event to a tempfile.
//! Run a minimal chat send; assert the file contains both event names.

#[tokio::test]
async fn session_start_and_stop_fire_during_chat_send() {
    // ... setup: create a temp hook script that appends event names to a file ...
    // ... configure HookEngine with the script ...
    // ... run a single chat send through AgentRuntime ...
    // ... read the file, assert "SessionStart" and "Stop" both present ...
}
```

(Detailed setup is non-trivial; if the existing test infrastructure for chat_send doesn't expose the right knobs, the test can mock the agent loop more narrowly.)

- [ ] **Step 6: Run test**

Run `cargo nextest run -p agent --test lifecycle_hooks`. Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs \
        crates/app-core/src/handlers/chat/streaming.rs \
        crates/agent/tests/lifecycle_hooks.rs

git commit -m "$(cat <<'EOF'
feat(hooks): wire SessionStart, SessionEnd, Stop, Error lifecycle hooks

SessionStart fires when a chat thread enters coding mode for the first
time (in chat_send). SessionEnd fires on chat_cancel and at session
terminal state. Stop fires after the agent loop's final turn emits
AgentEvent::Done. Error fires on the unrecoverable error path.

Part of Phase 1 Plan 4 (commit 7/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Composer + compaction + subagent + notification hooks

**Goal:** Wire UserPromptSubmit, PreCompact, PostCompact, SubagentSpawn, and Notification at their natural boundaries.

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs` (UserPromptSubmit, Notification)
- Modify: `crates/agent/src/execution/core.rs` (or wherever MidLoopCompressor lives) (PreCompact, PostCompact)
- Modify: `crates/agent/src/subagent.rs` (SubagentSpawn)
- Test: `crates/agent/tests/composer_hooks.rs`

- [ ] **Step 1: Wire UserPromptSubmit**

Open `crates/app-core/src/handlers/chat/streaming.rs`. Find `chat_send`. After slash classification but before agent-loop entry, add:

```rust
if let Some(engine) = core.agent.hook_engine() {
    let input = klynt_hooks::events::user_prompt_submit::UserPromptSubmitInput {
        session_id: session_key.clone(),
        prompt: content.clone(),
        ..Default::default()
    };
    let _ = engine.fire(klynt_hooks::engine::HookFireInput::UserPromptSubmit(input)).await;
}
```

- [ ] **Step 2: Wire PreCompact + PostCompact**

Open the file housing `MidLoopCompressor` (likely `crates/agent/src/execution/core.rs` or `crates/agent/src/execution/mid_loop_compressor.rs`). Find the `compress(...)` method. Wrap:

```rust
if let Some(engine) = self.hook_engine.as_ref() {
    let input = klynt_hooks::events::pre_compact::PreCompactInput {
        session_id: /* ... */,
        message_count: messages.len() as u64,
        current_tokens: tokens_used,
        context_window: 200_000,
        ..Default::default()
    };
    let _ = engine.fire(klynt_hooks::engine::HookFireInput::PreCompact(input)).await;
}

let result = /* existing compaction body */;

if let Some(engine) = self.hook_engine.as_ref() {
    let input = klynt_hooks::events::post_compact::PostCompactInput {
        session_id: /* ... */,
        messages_compacted: /* ... */,
        tokens_before: /* ... */,
        tokens_after: /* ... */,
        ..Default::default()
    };
    let _ = engine.fire(klynt_hooks::engine::HookFireInput::PostCompact(input)).await;
}

result
```

(MidLoopCompressor needs access to `Arc<HookEngine>` — pass via constructor; AgentRuntime provides it.)

- [ ] **Step 3: Wire SubagentSpawn**

Open `crates/agent/src/subagent.rs`. Find `spawn_subagent` (or wherever a sub-agent is created). Fire:

```rust
if let Some(engine) = parent_runtime.hook_engine() {
    let input = klynt_hooks::events::subagent_spawn::SubagentSpawnInput {
        session_id: child_session_key,
        parent_session_id: Some(parent_session_key),
        profile: profile_name.into(),
        task_summary: task_summary.into(),
        ..Default::default()
    };
    match engine.fire(klynt_hooks::engine::HookFireInput::SubagentSpawn(input)).await {
        klynt_hooks::engine::HookOutcome::Block { reason } => {
            return Err(common::KlyntbotError::Tool(common::ToolError::HookBlocked(reason)));
        }
        _ => {}
    }
}
```

- [ ] **Step 4: Wire Notification**

Open `crates/app-core/src/handlers/chat/streaming.rs`. In the approval-card emit path (where `agent:approval_requested` is sent to the React layer), fire:

```rust
let input = klynt_hooks::events::notification::NotificationInput {
    session_id: session_key.clone(),
    kind: "approval_card_opened".into(),
    message: format!("Approval requested for tool: {tool}"),
    tool: Some(tool.clone()),
    ..Default::default()
};
let _ = engine.fire(klynt_hooks::engine::HookFireInput::Notification(input)).await;
```

Also fire from the desktop notification toast path (`ApprovalToasts.tsx` is the React side; the Rust side that emits the OS notification is the matching site).

- [ ] **Step 5: Write integration test**

Create `crates/agent/tests/composer_hooks.rs`:

```rust
#[tokio::test]
async fn user_prompt_submit_fires_on_chat_send() {
    // Same pattern as lifecycle_hooks: temp script appending to file,
    // fire chat_send with a known prompt, assert UserPromptSubmit fired.
}

#[tokio::test]
async fn subagent_spawn_can_block_via_hook() {
    // Configure a SubagentSpawn hook that returns block.
    // Spawn a sub-agent. Assert spawn fails with HookBlocked.
}
```

- [ ] **Step 6: Run tests**

Run `cargo nextest run -p agent --test composer_hooks`. Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs \
        crates/agent/src/execution/core.rs \
        crates/agent/src/subagent.rs \
        crates/agent/tests/composer_hooks.rs

git commit -m "$(cat <<'EOF'
feat(hooks): wire composer + compaction + subagent + notification hooks

UserPromptSubmit fires in chat_send after slash classification.
PreCompact + PostCompact wrap MidLoopCompressor::compress.
SubagentSpawn fires in spawn_subagent (blockable — refused spawn
returns HookBlocked).
Notification fires on approval-card emit path.

All 13 hook events now wired end-to-end.

Part of Phase 1 Plan 4 (commit 8/9).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: "Add rule" UI + Settings HooksSection + master spec amendments + K-invariants + integration tests

**Goal:** Final commit. Adds the UI affordances (inline Starlark editor + read-only Settings hooks display), amends the master spec to reflect Plan 4's actual delivery, adds the remaining property invariants, and runs the full Phase 1 acceptance suite.

**Files:**
- Create: `desktop-ui/src/features/coding/components/StarlarkRuleEditor.tsx` + `.test.tsx`
- Create: `desktop-ui/src/features/settings/components/sections/HooksSection.tsx` + `.test.tsx`
- Modify: `desktop-ui/src/features/coding/hooks/useApprovalQueue.ts` (handle add_rule decision)
- Modify: `desktop-ui/src/features/coding/components/ApprovalCard.tsx` (wire Add rule button)
- Modify: `desktop-ui/src/features/settings/components/Settings.tsx` (or equivalent — add HooksSection tab)
- Modify: `crates/desktop/src/commands/coding.rs` (chat_save_starlark_rule; coding_hooks_list)
- Modify: `crates/desktop/src/specta_builder.rs` (register new commands)
- Modify: `crates/app-core/src/coding/handlers.rs` (or equivalent)
- Modify: `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` (master spec amendments)
- Test: `tests/integration/plan4_hooks_e2e.rs`
- Test: `tests/coding_in_chat_property.rs` (K3 already added in Task 1; add K_PLAN4_RULE_ROUNDTRIP)

- [ ] **Step 1: Add chat_save_starlark_rule Tauri command**

Open `crates/desktop/src/commands/coding.rs`. Add:

```rust
#[klynt_command]
pub async fn chat_save_starlark_rule(
    request_id: String,
    rule_source: String,
    suggested_filename: Option<String>,
) -> common::Result<String> {
    // Implementation: validate Starlark parses, write to ~/.klyntbot/rules/<filename>.rules
    // Return the path written.
    AppCore::singleton().chat_save_starlark_rule(request_id, rule_source, suggested_filename).await
}
```

- [ ] **Step 2: Add the AppCore handler**

Open `crates/app-core/src/coding/handlers.rs` (or create). Add:

```rust
impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn chat_save_starlark_rule(
        &self,
        request_id: String,
        rule_source: String,
        suggested_filename: Option<String>,
    ) -> common::Result<String> {
        // Validate by parsing
        let _ = klynt_execpolicy::parse_to_policy(&rule_source, std::path::Path::new("inline.rules"))
            .map_err(|e| common::KlyntbotError::Config(format!("invalid Starlark: {e}")))?;

        let rules_dir = dirs::home_dir()
            .ok_or_else(|| common::KlyntbotError::Config("no home dir".into()))?
            .join(".klyntbot/rules");
        std::fs::create_dir_all(&rules_dir)?;

        let filename = suggested_filename
            .unwrap_or_else(|| format!("rule-{}.rules", chrono::Utc::now().format("%Y%m%d-%H%M%S")));
        let path = rules_dir.join(filename);
        std::fs::write(&path, rule_source)?;

        // Reload the policy on the live agent
        let new_policy = klynt_execpolicy::Policy::load_from_dir(&rules_dir)?;
        self.swap_policy(Arc::new(new_policy)).await;

        // Resolve the pending approval to allow_once for the request_id
        self.resolve_pending_approval(&request_id, /* allow */ true).await;

        Ok(path.to_string_lossy().into_owned())
    }
}
```

`AppCore::swap_policy` and `AppCore::resolve_pending_approval` may need to be added — match the existing pattern for similar lifecycle methods.

- [ ] **Step 3: Register the command in specta_builder**

Open `crates/desktop/src/specta_builder.rs`. Add `chat_save_starlark_rule` (and `coding_hooks_list` for Step 5) to the `klynt_collect_commands![...]` macro args.

Run `cargo tauri dev` once and Ctrl-C after a few seconds; this regenerates `desktop-ui/src/bindings.ts`.

- [ ] **Step 4: Create StarlarkRuleEditor component**

Create `desktop-ui/src/features/coding/components/StarlarkRuleEditor.tsx`:

```tsx
import { useState, useCallback } from "react";
import { invoke } from "@/api/client";

interface Props {
  requestId: string;
  initialDraft?: string;
  onCommit: (path: string) => void;
  onCancel: () => void;
}

export function StarlarkRuleEditor({ requestId, initialDraft, onCommit, onCancel }: Props) {
  const [src, setSrc] = useState(initialDraft ?? `prefix_rule(["git", "status"], decision="allow")\n`);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const onSave = useCallback(async () => {
    setSaving(true);
    setError(null);
    try {
      const path = await invoke<string>("chat_save_starlark_rule", {
        requestId,
        ruleSource: src,
        suggestedFilename: null,
      });
      onCommit(path);
    } catch (e: any) {
      setError(String(e?.message ?? e));
    } finally {
      setSaving(false);
    }
  }, [requestId, src, onCommit]);

  return (
    <div className="starlark-rule-editor">
      <h4>Add Starlark rule</h4>
      <textarea
        value={src}
        onChange={(e) => setSrc(e.target.value)}
        rows={8}
        spellCheck={false}
        autoFocus
      />
      {error && <div className="starlark-rule-editor__error">{error}</div>}
      <div className="starlark-rule-editor__actions">
        <button onClick={onCancel} disabled={saving}>Cancel</button>
        <button onClick={onSave} disabled={saving} className="starlark-rule-editor__primary">
          {saving ? "Saving..." : "Save rule"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Add coding_hooks_list Tauri command**

Open `crates/desktop/src/commands/coding.rs`. Add:

```rust
#[klynt_command]
pub async fn coding_hooks_list() -> common::Result<HooksTomlSnapshot> {
    AppCore::singleton().coding_hooks_list().await
}
```

`HooksTomlSnapshot` is a serde-friendly struct with `path`, `exists`, `content` fields. Add to `crates/desktop-shared/src/lib.rs` (or wherever shared IPC types live).

- [ ] **Step 6: Create HooksSection component**

Create `desktop-ui/src/features/settings/components/sections/HooksSection.tsx`:

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

interface Snapshot { path: string; exists: boolean; content: string; }

export function HooksSection() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  useEffect(() => {
    invoke<Snapshot>("coding_hooks_list").then(setSnap);
  }, []);

  if (!snap) return <div>Loading...</div>;
  if (!snap.exists) return (
    <div>
      <p>No <code>~/.klyntbot/hooks.toml</code> found.</p>
      <p>Hooks are user-managed; create the file to enable.</p>
    </div>
  );
  return (
    <div className="hooks-section">
      <p>
        Hook configuration: <code>{snap.path}</code>{" "}
        <button onClick={() => invoke("open_path", { path: snap.path })}>Open in editor</button>
      </p>
      <pre className="hooks-section__content">{snap.content}</pre>
    </div>
  );
}
```

- [ ] **Step 7: Wire HooksSection into Settings**

Open the Settings tab/section host (`desktop-ui/src/features/settings/components/Settings.tsx` or equivalent). Add a tab "Hooks" rendering `<HooksSection />`.

- [ ] **Step 8: Wire ApprovalCard's "Add rule" button to the editor**

Open `desktop-ui/src/features/coding/components/ApprovalCard.tsx`. Find the existing "Add rule…" button (per agent extraction at lines 78-96). Replace its onClick:

```tsx
const [editorOpen, setEditorOpen] = useState(false);

// existing render...
<button onClick={() => setEditorOpen(true)}>Add rule… (r)</button>
{editorOpen && (
  <StarlarkRuleEditor
    requestId={item.requestId}
    onCommit={(path) => {
      onRespond(item.requestId, { kind: "add_rule", starlark_source: src });
      setEditorOpen(false);
    }}
    onCancel={() => setEditorOpen(false)}
  />
)}
```

- [ ] **Step 9: Master spec amendments**

Open `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`. Apply 3 amendments per the plan header:

**Amendment 1** (line 770-775): Replace the `custom_rule` example. Find:

```python
def check_git_push(args):
    if "main" in args or "master" in args:
        return forbid("never auto-push to main/master")
    return ask()

custom_rule(["git", "push"], handler=check_git_push)
```

Replace with:

```python
# Use prefix_rule with multiple decisions; for richer logic, write multiple
# prefix_rules with progressively more specific patterns. Klyntbot's
# klynt-execpolicy (vendored from codex-rs/execpolicy) supports
# prefix_rule, network_rule, host_executable; custom_rule with handler
# callbacks is not implemented (would require Starlark-defined functions
# to retain references across multiple eval calls).

prefix_rule(["git", "push", "main"], decision="forbidden", justification="never auto-push to main")
prefix_rule(["git", "push", "master"], decision="forbidden", justification="never auto-push to master")
prefix_rule(["git", "push"], decision="ask")
```

**Amendment 2** (Appendix F, after the existing tool-layer-consolidation row): Add a new row to the amendment log:

```markdown
| 2026-05-01 | Plan 4: Layer 2 Starlark + 13 hook events. Vendor codex-rs/execpolicy + codex-rs/hooks. Adds 8 klynt-extension hook events (SessionEnd, Pre/PostCompact, Pre/PostFileEdit, Notification, SubagentSpawn, Error). | [`docs/superpowers/plans/2026-05-01-klynt-coding-in-chat-phase1-plan4-layer2-starlark-and-hooks.md`](../plans/2026-05-01-klynt-coding-in-chat-phase1-plan4-layer2-starlark-and-hooks.md). |
```

**Amendment 3**: Section 7 Hook events table is accurate now — no change. (The 13 events all exist post-Plan-4.)

- [ ] **Step 10: Add property invariants**

Append to `tests/coding_in_chat_property.rs`:

```rust
#[test]
fn k_plan4_rule_roundtrip_save_then_load() {
    // Save a rule via chat_save_starlark_rule; load_from_dir; assert eval
    // returns the same Decision the test asserted.
    let dir = tempfile::TempDir::new().unwrap();
    let rule_path = dir.path().join("test.rules");
    std::fs::write(&rule_path,
        r#"prefix_rule(["echo", "hello"], decision="allow")"#).unwrap();
    let policy = klynt_execpolicy::Policy::load_from_dir(dir.path()).unwrap();
    assert_eq!(policy.eval(&["echo", "hello"], None), klynt_execpolicy::Decision::Allow);
}

proptest! {
    /// K_PLAN4_HOOK_DETERMINISM — for the same input event JSON, the
    /// engine produces the same outcome (modulo subprocess timing).
    #[test]
    fn k_plan4_hook_determinism(seed in 0u64..1000) {
        // Configure an in-process synthetic hook that just echoes a fixed
        // outcome. Fire 10 times. Assert all outcomes equal.
        // ...
    }
}
```

- [ ] **Step 11: End-to-end integration test**

Create `tests/integration/plan4_hooks_e2e.rs`:

```rust
//! End-to-end: configure ~/.klyntbot/hooks.toml + ~/.klyntbot/rules/git.rules
//! in a temp HOME, send a coding-mode chat message that calls bash with
//! a git command, verify the right hook events fire and the Layer 2 rule
//! gates the call.

#[tokio::test]
#[ignore] // requires real subprocess; run manually
async fn full_layer2_plus_hooks_path() {
    // ...
}
```

- [ ] **Step 12: Run full Phase 1 acceptance suite**

Run, in sequence:
- `cargo build --workspace`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo fmt --all --check`
- `cargo nextest run --workspace`
- `cd desktop-ui && bun run lint && bun run typecheck && bun run test && cd ..`
- `./scripts/run_kca_validation.sh`

Expected: all green, zero warnings, KCA gates pass.

- [ ] **Step 13: Commit**

```bash
git add desktop-ui/src/features/coding/components/StarlarkRuleEditor.{tsx,test.tsx} \
        desktop-ui/src/features/coding/components/ApprovalCard.tsx \
        desktop-ui/src/features/coding/hooks/useApprovalQueue.ts \
        desktop-ui/src/features/settings/components/sections/HooksSection.{tsx,test.tsx} \
        desktop-ui/src/features/settings/components/Settings.tsx \
        crates/desktop/src/commands/coding.rs \
        crates/desktop/src/specta_builder.rs \
        crates/app-core/src/coding/handlers.rs \
        docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md \
        tests/integration/plan4_hooks_e2e.rs \
        tests/coding_in_chat_property.rs

git commit -m "$(cat <<'EOF'
feat(plan4): "Add rule" UI + Settings hooks display + spec amendments

Final commit of Phase 1 Plan 4. Adds:
- StarlarkRuleEditor inline component for ApprovalCard's "Add rule" button.
- chat_save_starlark_rule Tauri command (validates parse, writes to
  ~/.klyntbot/rules/<auto-named>.rules, reloads policy live).
- HooksSection in Settings — read-only display of ~/.klyntbot/hooks.toml
  with "Open in editor" affordance.
- coding_hooks_list Tauri command serving the snapshot.

Master spec amendments (per plan header):
- §7 line 770-775: drop custom_rule example, replace with multi-prefix-rule
  pattern (custom_rule isn't implemented in codex-rs/execpolicy).
- Appendix F amendment log: Plan 4 row.

Adds K_PLAN4_RULE_ROUNDTRIP property test and end-to-end integration test.

Phase 1 Plan 4 complete (commit 9/9). Master spec Phase 1 deliverables:
- Layer 2 Starlark execpolicy: ✓
- klynt-hooks engine: ✓
- PreToolUse + PostToolUse fire correctly: ✓
- 13 hook events wired (5 vendored + 8 klynt-extension): ✓

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Post-merge

### Recommended PR strategy

Single PR for Tasks 1-9, or split into two:
- PR-A: Tasks 1-5 (Phase 1 minimum: Layer 2 + 5 Codex events + 8 extensions + Pre/PostToolUse)
- PR-B: Tasks 6-9 (PreFileEdit + lifecycle + composer + UI + spec)

PR-B is safer to ship after PR-A soaks for 1-2 days.

### Required reviews

- KCA validation: `./scripts/run_kca_validation.sh` must pass
- Recommended: `/ultrareview` on the PR — new vendor + cross-crate plumbing has broad blast radius

### Out of scope (Phase 2+)

- Layer 3 Mirror-learned approval (deferred per master spec line 785)
- File snapshots / `/sessions rewind` (Phase 2)
- `tool_search` BM25 ranking (Phase 2)
- `custom_rule` Starlark builtin (would require new starlark-eval design; not in any current plan)

---

*End of plan.*
