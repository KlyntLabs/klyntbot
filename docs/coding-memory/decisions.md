# Coding Memory — Decision Records

Decisions locked during brainstorming and carried forward into the spec
(`docs/superpowers/specs/2026-04-22-coding-memory-design.md`). One entry per
axis. Reversing any of these requires a new ADR.

| # | Axis | Decision | Rationale |
|---|---|---|---|
| 1 | Project shape | Combined memory + cognition + multi-CLI ingestion in one system | Single layer the agent reads through; avoids fan-out across two stores |
| 2 | Ingestion priority | External CLIs first; native `klynt-cli` integration deferred | Most users live in Claude Code / Codex / kimi-cli today; hooks beat SDK lock-in |
| 3 | Data topology | Approach A — shared SQLite + LanceDB store with klyntbot | Personal and coding memories enrich each other; one Reforge cycle for both |
| 4 | Crate placement | Two new L5 crates (`coding-memory`, `coding-ingest`) | Matches existing `agent` / `cognitive` / `channels` layering |
| 5 | Distiller timing | Per-turn batched (one LLM call per user turn, not per tool call) | Cost-bounded; batches naturally at the assistant's response boundary |
| 6 | Distiller role | Online writer only — emits ADD or SUPERSEDE; never DELETE | Write integrity is sacred (invariant 5) |
| 7 | Reforge role | Offline optimizer only — six responsibilities, never overwrites raw memory | Bi-temporal monotonicity (invariant 6) |
| 8 | Mirror role | Real-time observer; `PatternEffectivenessSubscriber` updates EMA in seconds | Effectiveness signal is the missing feedback loop in field state-of-the-art |
| 9 | Integration surface | Hooks (passive) + MCP tools (active). No LLM proxy, no ACP in Phase 1–7 | Lowest-friction install; no per-CLI SDK work |
| 10 | Daemon lifecycle | klyntbot desktop owns the ingest socket; hook falls back to file buffer when desktop is off | User can keep working with the CLI while desktop is closed; warning surfaces it |
| 11 | User install path | Desktop UI settings page (no separate CLI install command) | One install surface; toggle writes the CLI's own settings file atomically |
| 12 | Rule artifacts | Reforge writes managed-block sections of `CLAUDE.md`, `AGENTS.md`, `.cursorrules`; user hand-edits preserved outside markers | User authority over their own files; no silent overwrites |
| 13 | Schema approach | Consolidated Phase-1 migration for every column/table across all 8 phases | Pre-release authorizes direct DDL per CLAUDE.md; one migration story |
| 14 | klynt-cli source class | First-class native source emitting the rich variant set (10 net-new `EventKind`s beyond the 9 used by external CLIs) | klynt-cli has structured signals external CLIs don't expose; spec at `docs/superpowers/specs/2026-04-23-klynt-cli-design.md` |

## The nine invariants (enforced via `proptest`)

1. Provenance-always — every fact carries `metadata.provenance.source_events`.
2. Distiller-never-deletes — row count monotone non-decreasing per cycle.
3. Reforge-never-deletes-raw — all prior episodic rows survive any Reforge cycle.
4. Bi-temporal monotone — `valid_until >= valid_from`.
5. SUPERSEDE chain — predecessor's `valid_until == successor's valid_from`.
6. Scope isolation — repo-scoped retrieval never leaks cross-repo facts (except global `scope_repo_id IS NULL`).
7. Hook round-trip identity — `parse(serialize(AgentEvent)) == AgentEvent` for every CLI format.
8. Causal edge validity — no dangling `from_id` / `to_id` references.
9. Budget enforcement — SessionStart injection ≤ 800 tokens; UserPromptSubmit ≤ 1500 tokens.

## Hook handling (Phase 2 closure)

Claude Code emits 7 hooks; the adapter records all 7:

| Hook | EventKind |
|---|---|
| `SessionStart` | `SessionStart` |
| `UserPromptSubmit` | `UserPrompt` |
| `PreToolUse` | dropped — approval-layer only, not memorable |
| `PostToolUse` | dispatched: `Bash` test patterns → `TestRun`; `Edit`/`Write`/`MultiEdit` → `FileEdit`; `Read` → `FileEdit { op: Read }`; others → `ToolCall` |
| `Notification` | `Error { tool: None, message }` — preserves audit trail without inventing a variant |
| `Stop` | `AssistantMsg` — turn boundary marker |
| `SubagentStop` | `AssistantMsg` — semantically a turn boundary in nested context |
| `PreCompact` | `CompactEvent` |
