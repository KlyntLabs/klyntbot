# KlyntBot Architecture Documentation

> **The single source of truth for the KlyntBot codebase architecture.**
> If this disagrees with `CLAUDE.md`, `README.md`, or `AGENTS.md` at the repo root, **this wins** — those files lag and need a refresh pass. See [Document maintenance](#document-maintenance).
> **Last refreshed:** 2026-05-16 (commit `575b7014c`)

## Start here

If this is your first time, read **[`00-overview.md`](./00-overview.md)** end-to-end. It's the single file that gives you a complete mental model of the project — TL;DR, master Mermaid diagrams, subsystem inventory, critical-crate index, end-to-end workflow traces, glossary. Everything else hangs off it.

If you already know what you're looking for, the rest of this README is a navigation map.

## Audience guide

| You are… | Read this first |
|---|---|
| **External evaluator** | [`00-overview.md`](./00-overview.md) — TL;DR + the picture + cross-cutting findings |
| **New contributor (human)** | [`00-overview.md`](./00-overview.md) top to bottom, then pick a subsystem |
| **AI agent (Claude Code session)** | The subsystem inventory + critical-crate index in `00-overview.md`, then the specific doc |
| **Future-you (solo dev memory aid)** | [Cross-cutting findings](./00-overview.md#five-cross-cutting-findings) + [`TECH_DEBT.md`](./TECH_DEBT.md) |
| **About to PR against a specific crate** | The matching [`crates/<crate>.md`](#critical-crate-deep-dives-11) — these are method-level references |
| **Debugging a behavior you don't understand** | The matching [`subsystems/NN-*.md`](#subsystems-14) — these explain *how it works* with workflows |

## File map

### Top-level

| File | Purpose | Audience |
|---|---|---|
| [`README.md`](./README.md) (this file) | Navigation + doc-system maintenance | All |
| [`00-overview.md`](./00-overview.md) | Single-file mental model. 3 Mermaid diagrams + subsystem inventory + 5 cross-cutting findings + 3 end-to-end workflows + glossary | All — read first |
| [`TECH_DEBT.md`](./TECH_DEBT.md) | Living debt inventory. ~130 entries across 9 categories | All who code |

### Subsystems (14)

The navigation spine. Each doc covers a logical subsystem (which crates roll up together + how they interact + workflows + extension points).

| # | Doc | Status | Crates |
|---:|---|---|---|
| 01 | [`subsystems/01-foundations.md`](./subsystems/01-foundations.md) | 🟢 Stable | `common`, `config`, `bus` |
| 02 | [`subsystems/02-storage.md`](./subsystems/02-storage.md) | 🟢 Stable | `storage`, `session` |
| 03 | [`subsystems/03-providers.md`](./subsystems/03-providers.md) | 🟢 Stable | `providers` |
| 04 | [`subsystems/04-agent-runtime.md`](./subsystems/04-agent-runtime.md) | 🟡 In Progress | `agent`, `context_engine`, `skill-system` |
| 05 | [`subsystems/05-cognitive-memory.md`](./subsystems/05-cognitive-memory.md) | 🟡 In Progress | `cognitive`, `ai-core`, `ai-core-macros`, `autotuner` |
| 06 | [`subsystems/06-scheduling.md`](./subsystems/06-scheduling.md) | 🟡 In Progress | `scheduling` |
| 07 | [`subsystems/07-tools-framework.md`](./subsystems/07-tools-framework.md) | 🟢 Stable | `tools-core`, `tools-core-macros`, `tools` |
| 08 | [`subsystems/08-assistant-features.md`](./subsystems/08-assistant-features.md) | 🟢 Stable | 13 `feature-*` crates + `voice-engine` + `analytics` |
| 09 | [`subsystems/09-coding-mode.md`](./subsystems/09-coding-mode.md) | 🟡 In Progress | 14 crates: `klynt-*` + `coding-*` + `feature-coding-*` + `lsp-client` |
| 10 | [`subsystems/10-sandboxing-security.md`](./subsystems/10-sandboxing-security.md) | 🟢 Stable | `approval`, `klynt-sandbox`, `klynt-sandbox-helper`, `klynt-process-hardening` |
| 11 | [`subsystems/11-channels-mcp.md`](./subsystems/11-channels-mcp.md) | 🟡 In Progress | `channels`, `notifications`, `mcp`, `mcp-bridge`, `activity-log` |
| 12 | [`subsystems/12-plugins-platform.md`](./subsystems/12-plugins-platform.md) | 🟠 Scaffolded | `platform-input`, `platform-capture`, `platform-macos` |
| 13 | [`subsystems/13-desktop-frontend.md`](./subsystems/13-desktop-frontend.md) | 🟢 Stable | `desktop`, `desktop-shared`, `desktop-macros`, `crates/desktop-ui` *(stub)*, `/desktop-ui` *(repo root TS)*, `app-core`, `klyntbot`, `klyntbot-server` |
| 14 | [`subsystems/14-validation.md`](./subsystems/14-validation.md) | 🟠 Scaffolded | *none — chat-perf via `scripts/run_chat_perf_gates.sh`; LoCoMo + Letta wiring pending* |

### Critical-crate deep-dives (11)

Method-level references for the most-touched crates. Use these when you're about to PR against a specific crate.

| Crate | Doc | Status | Notes |
|---|---|---|---|
| `agent` | [`crates/agent.md`](./crates/agent.md) | 🟡 In Progress | Most-referenced doc — every constant + KCA env flag + ReAct loop pseudocode |
| `app-core` | [`crates/app-core.md`](./crates/app-core.md) | 🟢 Stable | The actual integration crate (NOT `klyntbot` facade); 14-phase init, ~40 handler domains |
| `cognitive` | [`crates/cognitive.md`](./crates/cognitive.md) | 🟡 In Progress | Longest doc. Includes "Common confusion points" up front for the name-collision gotchas |
| `coding-ingest` | [`crates/coding-ingest.md`](./crates/coding-ingest.md) | 🟡 In Progress | `AgentEvent::V1` shape + 5 adapters + hook CLI |
| `coding-memory` | [`crates/coding-memory.md`](./crates/coding-memory.md) | 🟡 In Progress | 3-phase Distiller + `ReforgeWriter` + 8 MCP recall tools |
| `context_engine` | [`crates/context_engine.md`](./crates/context_engine.md) | 🟢 Stable | `ContextEngine`, `BudgetAllocator`, `TieredHistoryCompressor`, `ContextSource` trait |
| `desktop` | [`crates/desktop.md`](./crates/desktop.md) | 🟢 Stable | 17-step startup + 5 secondary windows + 4 IPC-guard tests + OAuth flow |
| `mcp` | [`crates/mcp.md`](./crates/mcp.md) | 🟡 In Progress | Server bridges (`ToolRegistryBridge` + `AgentBridge`), client transports, sampling delegation |
| `providers` | [`crates/providers.md`](./crates/providers.md) | 🟢 Stable | `LlmProvider` trait + 4 adapters + circuit breaker + cache breakpoints |
| `storage` | [`crates/storage.md`](./crates/storage.md) | 🟢 Stable | `StoragePool`, full `Repos` aggregate (52 repos), `VectorStore`, `FinanceStorage` facade |
| `tools-core` | [`crates/tools-core.md`](./crates/tools-core.md) | 🟢 Stable | The trait surface every tool implements against |

## Cross-cutting facts to know

Pulled from `00-overview.md` for visibility. **Skip CLAUDE.md unless you've read these first.**

1. **The workspace has 62 crates** + the root `klyntbot` facade ≈ **63 crates**. CLAUDE.md and root README still say "39 crates / 9 layers" — stale.
2. **Multiple half-built features are documented as if shipped.** `lsp-client` is all-stubs. Notification channels for TG/DC/EM are unwired. MCP server-side approval always declines. Plugin `agent_ask_user` is a stub. Voice pronunciation pipeline is half-built. Computer Use platform layer is real but unwired. 4 Reforge phases in `coding-memory` are stubs.
3. **Migration debt visible in source.** Scheduling has two parallel runners. `storage` depends upward on `ai-core`. Legacy `messages.content` column mirrored on every write. `LEGACY_COMMAND_NAMES` dead-but-not-deleted. Stale "CronService" log message.
4. **`desktop-ui` location confusion.** `crates/desktop-ui/` is a Specta-generated bindings stub. The actual React frontend is at the repo root `/desktop-ui/`.
5. **5 coding-ingest adapters, not 4.** CLAUDE.md says 4 (`claude_code`, `codex`, `kimi_cli`, `opencode`); actual count includes `git_post_commit`. Plus only `claude_code` and `codex` are hook-driven — the others are poll-only.

## Status badge legend

Used at the top of every doc.

| Badge | Meaning | When |
|---|---|---|
| 🟢 **Stable** | Implemented, tested, in production use. Bug-fix territory only. | Default for shipped features. |
| 🟡 **In Progress** | Implemented but actively evolving. APIs may change. May have known gaps. | Features with open migration debt or phased rollouts. |
| 🟠 **Scaffolded** | Infrastructure exists but not wired to user-visible functionality. | E.g., `platform-capture` is real but no agent tool routes to it. |
| 🔴 **Stub** | Returns hardcoded/empty results. Marked `TODO`, `unimplemented!()`, or `NotImplementedInPhase`. | E.g., `lsp-client` methods. |
| ⚫ **Deprecated** | Replaced; awaiting deletion. Don't add to it. | E.g., `LEGACY_COMMAND_NAMES` const. |

Each doc carries a `Status last verified: YYYY-MM-DD` line. If it's older than a few months, treat with appropriate suspicion.

## Doc-system layout

```
docs/architecture/
├── README.md                     ← This file (index)
├── 00-overview.md                ← Single-file mental model
├── TECH_DEBT.md                  ← Living debt inventory (~130 entries)
├── subsystems/                   ← 14 subsystem docs (navigation spine)
│   ├── 01-foundations.md
│   ├── 02-storage.md
│   ├── 03-providers.md
│   ├── 04-agent-runtime.md
│   ├── 05-cognitive-memory.md
│   ├── 06-scheduling.md
│   ├── 07-tools-framework.md
│   ├── 08-assistant-features.md
│   ├── 09-coding-mode.md
│   ├── 10-sandboxing-security.md
│   ├── 11-channels-mcp.md
│   ├── 12-plugins-platform.md
│   ├── 13-desktop-frontend.md
│   └── 14-validation.md
└── crates/                       ← 11 critical-crate deep-dives
    ├── agent.md
    ├── app-core.md
    ├── cognitive.md
    ├── coding-ingest.md
    ├── coding-memory.md
    ├── context_engine.md
    ├── desktop.md
    ├── mcp.md
    ├── providers.md
    ├── storage.md
    └── tools-core.md
```

**28 markdown files, ~18,000 lines, 29 Mermaid diagrams, 60+ end-to-end workflow traces.**

## How each doc is structured

### Subsystem docs (`subsystems/NN-*.md`)

8-section pattern, in this order:

1. **TL;DR** — 2-4 sentences. External readers stop here.
2. **Architecture diagram** — Mermaid. The mental model.
3. **Mental model** — narrative. For new contributors.
4. **Reference** — file map, public API surface, key constants. For AI agents + future-you.
5. **Workflows** — numbered step-by-step traces. For all audiences.
6. **Internals** — implementation patterns, state machines, concurrency model. For AI agents + future-you.
7. **Dependencies & extension points** — how to plug in.
8. **Open questions / debt** — pointers to `TECH_DEBT.md`.

### Critical-crate docs (`crates/<crate>.md`)

Similar shape but tighter, focused on method-level reference:

1. **TL;DR** + status block
2. **Module map** — every file with one-line purpose
3. **Public API surface** — full type signatures, copy-paste-runnable
4. **Internals** or **Common confusion points** — the load-bearing facts that aren't obvious
5. **Workflows** — specific to the crate's responsibilities
6. **Testing approach** — how to test code that depends on this crate
7. **Extension points** — adding to this crate
8. **Key constants** + **Open questions** + **Cross-references**

## Document maintenance

### When docs go stale

Each of these triggers an update:

- A subsystem is added, removed, or significantly restructured.
- A new "critical crate" emerges (or a current one drops in importance).
- The status badge for any subsystem changes.
- A cross-cutting finding is resolved (e.g., layer-model drift gets fixed in CLAUDE.md).
- Reforge phase count, agent execution constants, or validation gate thresholds change.
- A previously-listed bug is fixed (move from TECH_DEBT to history).
- A new "Internals" pattern emerges that future readers need.

### How to update

1. Bump `Status last verified: YYYY-MM-DD` at the top of the affected doc.
2. Update the section that changed.
3. If a subsystem boundary moved, update the master Mermaid diagram in `00-overview.md` **and** the relevant subsystem doc(s).
4. If a critical-crate doc gets stale, update the per-crate doc — the index links to it.
5. If a TECH_DEBT.md item is closed, remove the row (the history is in git).
6. Run validation (see below).

### Validation

After substantive edits:

1. **Mentally run through the audience guide.** Does each audience still get what they need?
2. **Spot-check file:line references.** Files move; line numbers drift.
3. **Spot-check the `00-overview.md` cross-cutting findings.** If any are resolved, remove them.
4. **(Optional) Fresh-Claude reader test.** Open a fresh Claude conversation, paste a doc + a representative question, see if the answer is correct. If you need to fill in gaps from your own knowledge, the doc has a hole.

### Who owns this

The author of a code change owns the corresponding doc update. There is no separate "docs team." A PR that changes architecture without updating these docs should be considered incomplete.

### Don't write here without reading first

This doc system was built by reading the source extensively across multiple parallel scans. The single biggest mistake you can make is to write content based on "what should be true" rather than "what is true." When in doubt, read the code; if the doc disagrees, fix the doc.

## Related docs (outside this folder)

| Path | Purpose |
|---|---|
| Repo root `README.md` | Project pitch + install instructions. **Has stale "39 crates / 9 layers" claim.** |
| `CLAUDE.md` | Coding-agent guidance. **Out of date in several places** — this architecture system supersedes it. |
| `AGENTS.md` | Smoke-test instructions only. Not authoritative. |
| `CONTRIBUTING.md` | Workflow guidance for human contributors. |
| `SECURITY.md` | Threat model. |
| `CHANGELOG.md` | User-facing release notes (Keep-a-Changelog format). |
| `docs/superpowers/specs/` | Design specs for in-flight or planned features. |
| `docs/superpowers/plans/` | Implementation plans (more detailed than specs). |
| `docs/superpowers/notes/` | Research notes (less canonical than specs). |
| `scripts/run_chat_perf_gates.sh` | Chat-runtime perf gates (TTFT, throughput, coalescer). Replacement memory-quality gates (LoCoMo + Letta) wiring pending — see [`subsystems/14-validation.md`](./subsystems/14-validation.md). |
