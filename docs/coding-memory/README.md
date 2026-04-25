# Coding Memory

## Phase 2 — Ingestion transport + Claude Code E2E (shipped 2026-04-24)

Components newly live:

- `UnixIngestSocket` / `FileBufferFallback` — 200ms socket deadline; 50 MB rotate / 7 d TTL / 500 MB hard cap for the cold path.
- `HookClient` — socket-first-else-buffer dispatcher with rate-limited stderr warnings.
- `IngestDaemon` — binds `~/.klyntbot/ingest.sock`, decodes length-prefixed JSON, persists rows to `ingest_event_log`, drains any pre-existing buffer on startup, heartbeats `desktop.lock` every 30 s.
- Claude Code adapter — 7 hook events (`SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact`). Bash + test-framework detection emits `TestRun`; file-ops emit `FileEdit`.
- `ClaudeCodeInstaller` — idempotent `~/.claude/settings.json` merge with a pre-install backup; the `klyntbot-managed` matcher tag lets users keep their own hooks alongside.
- Workbench: Coding CLI settings page (toggle + Diagnose), CLI Health panel, Session Replay panel.

Unchanged: Distiller / Recall / Reforge / Mirror coding behavior all remain Phase 1 stubs. No facts are written to `semantic_facts` or `episodic_memories` yet — only `ingest_event_log` rows accumulate.

Exit-gate evidence: `tests/integration/coding_memory_phase2_roundtrip.rs`, `tests/integration/coding_memory_phase2_desktop_off.rs`.
