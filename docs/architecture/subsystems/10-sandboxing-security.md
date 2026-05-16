# Subsystem 10 — Sandboxing & Process Hardening

> **Status:** 🟢 Stable
> **Status last verified:** 2026-05-16
> **Crates:** `approval`, `klynt-sandbox`, `klynt-sandbox-helper`, `klynt-process-hardening`
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

Three layers of defense, executed in this order on every potentially dangerous operation:

1. **Pre-main hardening** (`klynt-process-hardening`) — runs as the *first* statement in `desktop/src/main.rs` (line 112). Denies debugger attach, disables core dumps, scrubs dangerous env vars. Must precede mimalloc init (line 114) because mimalloc reads `MALLOC_*` env vars.
2. **Approval gate** (`approval`) — every tool call passes through `ApprovalGate::check`. Three layers: `Tool::approval_class` declares baseline, `ClassifyHook`s (especially `CodingApprovalPolicy`) can upgrade/downgrade, persistent grants short-circuit prompts.
3. **OS sandbox** (`klynt-sandbox` + `klynt-sandbox-helper`) — bash and other subprocess execution runs under macOS Seatbelt (`.sbpl` via `sandbox-exec`) or Linux Landlock + bwrap (defense-in-depth, both layers run together when bwrap is available).

Four approval channels exist: **Desktop modal** (Tauri), **Telegram** (real bidirectional interactive approval), **MCP server** (always declines — placeholder), and **BlockingFallback** (always declines — used by channels without UI). The four `ApprovalClass` levels (`Safe`/`Sensitive`/`Destructive`/`Admin`) drive remote-channel auto-allow heuristics.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef hard fill:#ffcdd2,stroke:#c62828,color:#b71c1c
    classDef gate fill:#fbe9e7,stroke:#d84315,color:#bf360c
    classDef sandbox fill:#ffe0b2,stroke:#ef6c00,color:#e65100
    classDef channel fill:#fff,stroke:#999,stroke-dasharray:5

    PMH[pre_main_hardening<br/><i>FIRST statement in main()</i><br/>ptrace deny · RLIMIT_CORE=0<br/>env scrub LD_/DYLD_/Malloc*]:::hard

    AG[ApprovalGate<br/><i>check(req, cancel_token) → GateOutcome</i><br/>Allow / Deny / Cancel]:::gate
    CH[ClassifyHook chain<br/><i>last non-None wins</i>]:::gate
    CAP[CodingApprovalPolicy<br/><i>Default / PlanMode / YoloMode</i>]:::gate
    GR[(approval_grants<br/><i>session or forever</i>)]:::gate
    HI[(coding_approval_history<br/><i>append-only audit</i>)]:::gate

    SBX[klynt-sandbox<br/><i>SandboxRunner trait</i>]:::sandbox
    MS[MacOsSeatbeltRunner<br/><i>sandbox-exec + .sbpl<br/>3 template substitutions</i>]:::sandbox
    LS[LinuxSandboxRunner<br/><i>bwrap + Landlock<br/>defense-in-depth</i>]:::sandbox
    HLP[klynt-sandbox-helper<br/><i>Linux-only child binary<br/>no_new_privs + Landlock + execvp<br/>exit codes 124/125/126</i>]:::sandbox

    DC[DesktopApprovalChannel<br/><i>Tauri modal · oneshot · 600s</i>]:::channel
    TC[TelegramApprovalChannel<br/><i>full interactive</i>]:::channel
    MC[McpApprovalChannel<br/><i>ALWAYS DECLINES (stub)</i>]:::channel
    BC[BlockingFallbackChannel<br/><i>ALWAYS DECLINES</i>]:::channel

    PMH --> AG
    CH --> AG
    CAP --> CH
    GR --> AG
    AG --> HI
    AG --> SBX
    SBX --> MS
    SBX --> LS
    LS --> HLP
    AG --> DC
    AG --> TC
    AG --> MC
    AG --> BC
```

---

## Mental model

**Defense in depth, three rings:**

```
┌───────────────────────────────────────────────────────────┐
│ Ring 0 — Process boundary                                 │
│   pre_main_hardening                                      │
│   • ptrace(PT_DENY_ATTACH) on macOS — no debugger attach  │
│   • RLIMIT_CORE = 0 — no core dumps                       │
│   • Scrub LD_*/DYLD_*/MallocStackLogging* env vars        │
│   Runs BEFORE allocator init                              │
└───────────────────────────────────────────────────────────┘
              │
              ▼
┌───────────────────────────────────────────────────────────┐
│ Ring 1 — Tool approval                                    │
│   ApprovalGate                                            │
│   • Tool::approval_class(args) baseline                   │
│   • ClassifyHook chain (last non-None wins)               │
│   • CodingApprovalPolicy: Default/PlanMode/YoloMode       │
│   • Persistent grants short-circuit (session or forever)  │
│   • Remote channels: auto-allow Sensitive; prompt for     │
│     Destructive/Admin only                                │
└───────────────────────────────────────────────────────────┘
              │
              ▼
┌───────────────────────────────────────────────────────────┐
│ Ring 2 — OS sandbox                                       │
│   macOS:  sandbox-exec -p <sbpl> <prog>                   │
│   Linux:  bwrap (namespaces) → klynt-sandbox-helper       │
│           (no_new_privs + Landlock + seccomp) → execvp    │
│   (bwrap and Landlock both run when bwrap available)      │
└───────────────────────────────────────────────────────────┘
```

**Key distinction:** approval = "should this happen?" Sandbox = "make sure this *only* does what it's allowed to do." Both are needed; neither replaces the other.

---

## Reference

### `approval` — file map

| Path | Purpose |
|---|---|
| `src/lib.rs` | Re-exports |
| `src/gate.rs` | `ApprovalGate`, `GateOutcome`, `check()` |
| `src/policy.rs` | `ClassifyHook` trait |
| `src/coding_policy.rs` | `CodingApprovalPolicy` (3 variants) |
| `src/channel.rs` | `ApprovalChannel` trait + `BlockingFallbackChannel` |
| `src/grants.rs` | `GrantRow`, `GrantRepo` (CRUD on `approval_grants` table) |
| `src/gate.rs` | `ApprovalSuggester` trait (mirror feedback) |

### `ApprovalGate`

```rust
pub struct ApprovalGate { ... }
impl ApprovalGate {
    pub fn new(grants: Arc<GrantRepo>, channel: Arc<dyn ApprovalChannel>) -> Self;
    pub fn with_classify_hooks(self, hooks: Vec<Arc<dyn ClassifyHook>>) -> Self;
    pub fn with_suggester(self, suggester: Arc<dyn ApprovalSuggester>) -> Self;

    pub async fn check(&self, req: ApprovalRequest, cancel: &CancellationToken) -> Result<GateOutcome>;
}

pub enum GateOutcome {
    Allow,
    Deny  { reason: String },
    Cancel,
}
```

The `check()` future races against `cancel_token` so a hung modal cannot block the agent loop indefinitely.

### `ClassifyHook` trait — last non-None wins

```rust
pub trait ClassifyHook: Send + Sync {
    fn classify(&self, tool: &str, action: Option<&str>, args: &Value) -> Option<ApprovalClass>;
    fn scope(&self, tool: &str, action: Option<&str>, args: &Value) -> Option<ApprovalScope> { None }
}
```

Gate iterates all registered hooks; **last non-`None` return wins** for both `classify` and `scope`. This allows layered overrides without requiring any single hook to be authoritative.

### `CodingApprovalPolicy` — 3 variants

| Variant | Behavior |
|---|---|
| `Default { allow, deny, ask, default_if_no_match }` | Compiled glob rules from `CodingPermissions` config. Order: deny → allow → ask, then fallback. Tool name normalization strips `_`/`-` and lowercases. Tools without a resource arg return `None` (no opinion). |
| `PlanMode { plan_file_path, ... }` | Write tools `Destructive` unless targeting exactly the plan file. Read tools always `Safe`. Bash always `Destructive`. Unknown/MCP tools `Destructive`. |
| `YoloMode { until }` | Returns `Safe` for all calls until the `until` timestamp expires. After expiry: falls through to ask-everything (because the hardcoded fallback `matches!(DefaultPolicy::Ask, DefaultPolicy::Allow)` is `false`). |

Glob rule shape: `Tool(glob)`, e.g. `bash(ls *)`, `read(src/**)`. Tool name normalization (strip + lowercase) means `BashTool` and `bash-tool` and `bash_tool` all match `bash`.

### `ApprovalChannel` implementations

| Channel | File | Behavior |
|---|---|---|
| `DesktopApprovalChannel` | `app-core/src/desktop_approval_channel.rs` (+ thin wrapper in `desktop/src/approval/`) | Holds `DashMap<request_id, oneshot::Sender>`. `request()` parks on oneshot (600s timeout). `respond_approval()` wakes it. Emits `approval-requested` Tauri event to frontend. |
| `TelegramApprovalChannel` | `channels/src/adapters/telegram_approval.rs` | **Real interactive approval.** Sends Telegram message with inline keyboard, awaits reply via the channel's bidirectional flow. Fourth concrete implementation alongside Desktop/BlockingFallback/Mcp. |
| `McpApprovalChannel` | `mcp/src/server/approval.rs` | **Always declines** — wraps a structured JSON error: `{"code": "approval-required", "tool": "...", "action": "...", "class": "...", "message": "..."}`. Caller (`deny_to_mcp_error`) translates to MCP error. Stub. |
| `BlockingFallbackChannel` | `approval/src/channel.rs` | **Always declines** — `"Action requires approval. Open Klynt on desktop to confirm."` Used by channels without UI. `capabilities()` claims `supports_classes: {Destructive, Admin}` but answer is always Decline. |

### `approval_grants` schema

```sql
CREATE TABLE approval_grants (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    class        TEXT NOT NULL CHECK (class IN ('safe','sensitive','destructive','admin')),
    tool_name    TEXT NOT NULL,
    action       TEXT,
    resource_key TEXT,
    lifetime     TEXT NOT NULL CHECK (lifetime IN ('session','forever')),
    session_id   TEXT,
    granted_at   INTEGER NOT NULL,
    expires_at   INTEGER,
    UNIQUE (class, tool_name, action, resource_key, lifetime, session_id)
);
```

- **No `'once'` grant** — once-decisions return Allow immediately without persisting.
- **`'session'` rows** carry the session UUID; bulk-deleted by `purge_session()` on session end.
- **`'forever'` rows** have `session_id = NULL`.
- **`INSERT OR IGNORE`** on the unique key → session-grant insertion is idempotent.

### `coding_approval_history` schema

```sql
CREATE TABLE coding_approval_history (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    tool       TEXT NOT NULL,
    args_hash  TEXT NOT NULL,
    repo_id    TEXT NOT NULL DEFAULT '',
    decision   TEXT NOT NULL,    -- 'allow' | 'deny'
    decided_by TEXT NOT NULL,    -- 'user' | 'auto_allow' | 'auto_deny' | 'timeout' | 'cancelled'
    layer      TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as integer))
);
```

Append-only audit log — feeds Mirror learning Layer 3 (`ApprovalHistorySource`).

### `approval_pattern_history`

Path-based pattern log for Mirror suggestion training. Columns: `user_id, tool_name, path, decision, pattern_used`.

### `klynt-sandbox` — macOS Seatbelt

`MacOsSeatbeltRunner::build_sandboxed_command(policy, program, args)` returns a `tokio::process::Command` ready to spawn. Used both by foreground tool execution and by `feature-coding-bash`'s background PTY supervisor (which needs the raw `Command` handle).

**`.sbpl` template substitutions** (3 placeholders):

| Placeholder | Substitution |
|---|---|
| `{{CWD}}` | Canonicalized working directory |
| `{{EXTRA_WRITES}}` | `(deny file-write*)` for `ReadCwdOnly` or `None`; empty for `WriteCwdReadAll` |
| `{{NETWORK}}` | `(allow network*)` or `(deny network*)` |

**Base policy:** `(deny default)`, then allows process-fork, process-exec, signal-self, sysctl-read, mach-lookup, IPC shared-mem reads, global file-read, PTY device read+write+ioctl.

**Invocation:**
```
/usr/bin/sandbox-exec -p <rendered-sbpl> <program> <args...>
```

stdout+stderr are *merged* into `CommandOutput::stdout` field. Timeout enforced with `tokio::time::timeout`; on expiry child is killed and `SandboxError::ChildExit(124)` is returned.

### `klynt-sandbox` — Linux Landlock + bwrap

`LinuxSandboxRunner` detects mode at construction: probes `which bwrap` (Landlock availability is confirmed by helper exit code 125 at runtime). Three states:

| Mode | Available? |
|---|---|
| `WithBwrap` | bwrap binary present; both bwrap and Landlock run |
| `LandlockOnly` | no bwrap; Landlock via helper alone |
| `Unavailable` | neither available; cannot sandbox |

Helper binary located by checking `<parent_exe_dir>/klynt-sandbox-helper` first, then `PATH`.

**bwrap flags always added:** `--unshare-user --unshare-pid --die-with-parent --new-session`. Adds `--unshare-net` for `NetworkConstraints::Block`. System dirs (`/usr`, `/lib`, `/lib64`, `/bin`, `/sbin`, `/etc`) are `--ro-bind`ed. `/proc` and `/dev` virtualized. `/tmp` is fresh tmpfs. CWD `--bind` (rw) for `WriteCwdReadAll`, `--ro-bind` for `ReadCwdOnly`.

**Helper invocation patterns:**

`WithBwrap` mode:
```
/usr/bin/bwrap <bwrap-args> -- klynt-sandbox-helper --landlock <b64-policy> -- <program> <args>
```

`LandlockOnly` mode:
```
klynt-sandbox-helper --landlock-only <b64-policy> -- <program> <args>
```

Policy is JSON-serialized → base64-encoded (`STANDARD_NO_PAD`) → passed as single positional arg. Helper validates flag matches policy `mode`.

### `klynt-sandbox-helper` — Linux child binary

Sequence inside `fn main()` (Linux path):
1. Parse CLI → `ParsedArgs { policy, program, args }`
2. `apply_no_new_privs()` — `prctl(PR_SET_NO_NEW_PRIVS, 1)`. Failure → exit 126
3. `apply_landlock(&policy.sandbox)` — constructs Landlock ruleset, calls `restrict_self()`
4. If mode is `LandlockOnly` and ruleset is not `FullyEnforced` → exit 125. **In `WithBwrap` mode, exit 125 also propagates** — bwrap's process group exits with the helper's code, so the parent sees 125 and reports the sandbox as unavailable for that command. There is no graceful fall-through to bwrap-only.
5. `Command::new(program).args(args).exec()` — `execvp`; returns only on failure → exit 126

**On `seccomp`:** the architecture diagram lists `no_new_privs + Landlock + seccomp` as the Linux helper's defense stack, but **seccomp is not actually applied today** — only `no_new_privs` and Landlock are. The seccomp slot is reserved for a future filter; until then the diagram is aspirational on that one layer. Treat the architecture diagram's "seccomp" mention as a future hook, not a current behavior.

**Reserved exit codes:**

| Code | Meaning |
|---|---|
| 2 | non-Linux platform (helper exits early) |
| 124 | timeout (set by parent kill) |
| 125 | Landlock unavailable in `LandlockOnly` mode |
| 126 | setup failure (no_new_privs failed, or execvp returned) |

**`VENDOR.md`** in the crate documents provenance: adapted from `codex-rs/linux-sandbox/` under Apache-2.0. The stale `main.rs:1-4` comment still says "Plan 1: stub" — Plan 3 is fully active.

### `klynt-process-hardening`

```rust
pub fn pre_main_hardening() {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pre_main_hardening_linux();

    #[cfg(target_os = "macos")]
    pre_main_hardening_macos();

    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    pre_main_hardening_bsd();

    #[cfg(target_os = "windows")]
    pre_main_hardening_windows();   // TODO stub
}
```

| Platform | Behavior |
|---|---|
| macOS | `ptrace(PT_DENY_ATTACH, 0, ptr::null_mut(), 0)`. Failure → exit 6. Plus `RLIMIT_CORE = 0`. Plus scrub `DYLD_*`, `MallocStackLogging`, `MallocLogFile`. |
| Linux | `prctl(PR_SET_DUMPABLE, 0)`. Failure → exit 5. Plus `RLIMIT_CORE = 0`. Plus scrub `LD_*`. |
| BSD | `RLIMIT_CORE = 0`. Plus scrub `LD_*`. |
| Windows | Empty function. `// TODO: Windows hardening (Job Object, mitigations) is out of scope for Phase 3.` |

**Build position:** Called as the **first substantive statement** in `desktop/src/main.rs::main()` (after the `--hook` short-circuit). Runs before `configure_mimalloc()`, `Cli::parse()`, and `AppCore::init`. **The order is critical** — mimalloc reads `MALLOC_*` env vars at init, so the scrub must precede it.

---

## Workflows

### A bash command in coding mode passes through (all 3 rings)

```
Ring 1 — Approval
1. Agent loop emits bash tool call: { "command": "rm -rf /tmp/x" }
2. ApprovalGate.check(req, cancel_token):
   - ClassifyHook chain — last non-None wins
     CodingApprovalPolicy::Default extracts command, matches `bash(rm *)` deny glob
     → Returns Some(Destructive), Some(ToolActionResource("rm -rf /tmp/x"))
   - Class != Safe, no auto-allow
   - approval_grants lookup: no existing grant for this exact (tool, resource, lifetime, session)
   - Send to DesktopApprovalChannel:
     • Insert pending entry keyed by UUID
     • Emit `approval-requested` Tauri event with class+scope+suggestion
     • Park on oneshot::Receiver (600s timeout)
3. User clicks "Allow this session" in modal:
   - Tauri `approval_respond` command → core.respond_approval(id, AllowAlways)
   - DesktopApprovalChannel.resolve() → sends ApprovalDecision::Session on oneshot
4. Gate resumes:
   - Persist GrantRow { lifetime: Session, session_id: "..." } into approval_grants
   - Append row to coding_approval_history (decision=allow, decided_by=user)
   - Return GateOutcome::Allow

Ring 2 — OS Sandbox
5. (macOS) MacOsSeatbeltRunner.build_sandboxed_command(policy, "/bin/sh", &["-c", "rm -rf /tmp/x"])
   → /usr/bin/sandbox-exec -p <sbpl> /bin/sh -c 'rm -rf /tmp/x'
   - sbpl: (deny default) + base allows + {{NETWORK}} + {{EXTRA_WRITES}}
   - Spawned as tokio child; timeout via tokio::time::timeout
   - On timeout: kill + return SandboxError::ChildExit(124)
   - stdout+stderr merged into CommandOutput::stdout

   (Linux WithBwrap mode)
   LinuxSandboxRunner.spawn:
   → /usr/bin/bwrap <ns-args> --ro-bind /usr /usr --bind <cwd> <cwd>
     -- klynt-sandbox-helper --landlock <b64> -- /bin/sh -c 'rm -rf /tmp/x'
   - Helper: prctl(PR_SET_NO_NEW_PRIVS) → apply Landlock → execvp
```

### Process hardening at startup

```
desktop main()
   ├─ if argv[1] == "--hook" → hook_cli::run() → exit (no hardening needed for the hot path)
   │
   ├─ klynt_process_hardening::pre_main_hardening()                  [line 112]
   │     macOS:  ptrace(PT_DENY_ATTACH) + RLIMIT_CORE=0
   │             + scrub DYLD_*, MallocStackLogging, MallocLogFile
   │     Linux:  prctl(PR_SET_DUMPABLE, 0) + RLIMIT_CORE=0
   │             + scrub LD_*
   │
   ├─ configure_mimalloc()                                            [line 114]
   │     ← allocator now safe to read env (Malloc* already scrubbed)
   │
   ├─ Cli::parse()
   │
   └─ run_desktop_app()
         ├─ AppCore::init_with_sender()
         └─ tauri::Builder::default()...run()
```

### MCP server-side approval (always declines)

```
1. Remote MCP client (e.g., Claude Code on another machine) calls a sensitive tool
2. ToolRegistryBridge → tool.execute() → tool calls ApprovalGate
3. McpApprovalChannel.request():
   - BlockingFallbackChannel.desktop_prompt() returns Decline
   - Wrap in JSON: { "code": "approval-required", "tool": "...", "action": "...", "class": "...", "message": "Open Klynt on desktop to confirm." }
4. Gate returns GateOutcome::Deny { reason: <JSON> }
5. Caller (deny_to_mcp_error) parses JSON into structured McpError
6. Remote client sees structured error pointing to desktop
```

**This is intentional but limiting.** Remote MCP clients can never receive interactive prompts; they must redirect to the desktop. See [Open questions](#open-questions--debt).

---

## Internals

### Why hardening must precede mimalloc

mimalloc inspects `MALLOC_*` / `MallocStackLogging*` env vars at initialization. If they aren't scrubbed *before* mimalloc init, an attacker who can set env vars in the parent process can manipulate allocator behavior (potentially leaking memory layout via stack logs). The sequencing at `desktop/src/main.rs:112-114` is correct and load-bearing.

**Do not reorder these two calls.** A "let's init the allocator early for perf" refactor would silently break the hardening.

### Defense-in-depth: bwrap AND Landlock together

In `WithBwrap` mode, both run:
- **bwrap** provides user/pid/net namespace isolation (process can't see other processes, can't see real network)
- **Landlock** layers an additional FS restriction inside the bwrap container (process can read/write only the allowed paths)

If bwrap is unavailable, `LandlockOnly` provides FS isolation only. If Landlock is unavailable, the helper exits 125 and the runner falls back to `Unavailable` — no sandboxing.

### The "last non-None wins" hook chain

The `ClassifyHook` chain is iterated in registration order; for each hook, `classify()` is called. The final non-`None` result wins. This pattern lets you stack policies:

1. Base: tool declares its own `approval_class`
2. CodingApprovalPolicy: may upgrade based on file path (e.g., `read(/etc/passwd)` → Sensitive)
3. Custom user policy: could downgrade to Safe for explicitly trusted patterns

A hook that should never override returns `None`.

### `INSERT OR IGNORE` for grant idempotency

Session grants are inserted with `INSERT OR IGNORE` on the unique key. So if the same grant is granted twice in a session (e.g., user clicks "Allow always" on a previously-allowed action), the second insert silently no-ops. No errors, no duplicate rows.

### YoloMode expiry edge case

```rust
match policy {
    YoloMode { until } if Timestamp::now() < until => Some(ApprovalClass::Safe),
    _ => /* fall through */ None,
}
```

When `until` passes, YoloMode returns `None` from `classify`. The fallback logic in `Default` then applies — but the hardcoded fallback is `matches!(DefaultPolicy::Ask, DefaultPolicy::Allow)` which is `false`. **So expired YoloMode effectively becomes "ask everything," not "go back to Default."** Subtle; document loudly.

### Glob rule normalization

Tool name matching strips `_` and `-` and lowercases. So `BashTool`, `bash-tool`, `Bash_Tool`, and `bash` all match a `bash(...)` glob. Resource matching is full Unix-glob (via the `globset` crate).

---

## Dependencies & extension points

### Upstream deps

- `tools-core` (`ApprovalClass`, `ApprovalScope`)
- `common` (`SessionKey`, channel constants)
- `bus` (publishes approval events on `DomainEventBus` for mirror feedback)
- `storage` (`approval_grants`, `coding_approval_history` repos)
- `libc` (macOS/Linux syscalls for hardening)
- `globset` (glob matching)
- `tokio` (async runtime, oneshot channels)

### Adding a new `ApprovalChannel`

1. Implement `ApprovalChannel` trait — `request(req) -> ApprovalDecision`, `capabilities() -> Capabilities`.
2. Wire into `app-core::init` — register with the agent's `ApprovalGate`.
3. Decide capabilities:
   - `supports_classes` — which classes you can prompt for.
   - `supports_action_responses` — can you do anything beyond Allow/Deny (e.g., AllowOnce/AllowSession/AllowForever)?
4. If interactive: implement a bidirectional flow (see Telegram).
5. If non-interactive: return a structured Decline pointing user to desktop.

### Adding a `ClassifyHook`

1. Implement the trait. Return `None` to abstain, `Some(...)` to override.
2. Register via `gate.with_classify_hooks(hooks)` or push into existing chain.
3. **Order matters** — last non-None wins.

### Modifying the Seatbelt template

1. Edit `crates/klynt-sandbox/seatbelt_template.sbpl`.
2. Variables `{{CWD}}`, `{{EXTRA_WRITES}}`, `{{NETWORK}}` are the only substitutions; add new ones in `render_seatbelt_profile`.
3. Profile is `(deny default)` first — explicitly allow everything you need.
4. Test on real macOS — sandbox-exec error messages are sometimes opaque.

### Modifying the Linux Landlock policy

1. Edit `klynt-sandbox-helper/src/landlock_apply.rs::apply_landlock`.
2. Re-derive the policy struct in `klynt-sandbox::policy::Policy` (it's serialized + base64'd over the CLI).
3. Test under WSL or a Linux VM — Landlock semantics depend on kernel version (5.13+).

### Adding to the hardening sequence

1. Decide platform (macOS / Linux / BSD).
2. Add to the platform-specific function in `crates/klynt-process-hardening/src/lib.rs`.
3. Pick a unique exit code for failure — current codes: 5 (Linux PR_SET_DUMPABLE), 6 (macOS PT_DENY_ATTACH), 7 (any RLIMIT_CORE).
4. **Place it before mimalloc init.** If you add env-var inspection or anything that touches global state, double-check the sequence.

---

## Open questions & debt

- **`McpApprovalChannel` always declines.** Real MCP clients cannot get interactive approval. Either implement a callback-based protocol (sampling delegation in reverse?) or leave as-is and document the limitation prominently in MCP-related material.
- **`BlockingFallbackChannel.capabilities()` claims it supports Destructive/Admin** but always declines. Either tighten capabilities to advertise correctly, or change the channel name to convey "I will decline but you should still ask me."
- **`pre_main_hardening_windows()` is a stub.** Acknowledged non-goal per `lib.rs:95`. If Klyntbot ever expands to Windows, this needs Job Object + mitigations work.
- **YoloMode expiry edge case** (expired → "ask everything") is non-obvious. Either document loudly or change semantics to "fall back to Default policy."
- **Seatbelt `.sbpl` template is small (3 substitutions)** but the actual permission model lives in the base `(deny default)` + allow rules. Refactor candidates exist for richer policy expression.
- **"Plan 1 stub" comment in `klynt-sandbox-helper/src/main.rs:1-4`** is stale — Plan 3 is fully active. Update the doc comment.
- **`coding_approval_history`** grows unboundedly. Add a retention policy (e.g., 90 days).
- **No test coverage for `Telegram`/`MCP` approval channels** at the integration level — they're tested in isolation but not end-to-end against the gate.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 (stubs), #4 (stale refs) for specifics.

---

## Cross-references

- [`01-foundations.md`](./01-foundations.md) — `KlyntbotError::PermissionDenied`, `KlyntbotError::Cancelled`
- [`02-storage.md`](./02-storage.md) — `approval_grants`, `coding_approval_history`, `approval_pattern_history`
- [`07-tools-framework.md`](./07-tools-framework.md) — `ApprovalClass`, `ApprovalScope`, `Tool::approval_class` + `approval_scope`
- [`09-coding-mode.md`](./09-coding-mode.md) — bash/edit/write tool integration with sandbox
- [`11-channels-mcp.md`](./11-channels-mcp.md) — `TelegramApprovalChannel`, `McpApprovalChannel` consumer side
- [`13-desktop-frontend.md`](./13-desktop-frontend.md) — Tauri modal UI for `DesktopApprovalChannel`
