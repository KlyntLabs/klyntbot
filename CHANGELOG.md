# Changelog

All notable changes to Klyntbot are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## How to update this file

Klyntbot uses [Conventional Commits](https://www.conventionalcommits.org/). When you open a PR, add an entry to the `[Unreleased]` section using this mapping:

| Commit type | Changelog section |
|-------------|-------------------|
| `feat:` | Added |
| `fix:` | Fixed |
| `perf:`, `refactor:` (user-visible) | Changed |
| `feat!:` / `BREAKING CHANGE:` | Changed (mark as **BREAKING**) |
| Removing a feature | Removed |
| Marking something for future removal | Deprecated |
| Vulnerability fix | Security (link to advisory) |

Pure-internal changes (`chore:`, `docs:`, `test:`, `ci:`, `build:`, internal `refactor:`) do **not** require a changelog entry.

Reference issues and PRs as `(#123)`. Group related entries under sub-bullets when helpful.

---

## [Unreleased]

The upcoming `0.1.0` release — the first public release of Klyntbot, a local-first personal cognitive agent OS for macOS. On release this section will be renamed to `[0.1.0] — YYYY-MM-DD` and a fresh empty `[Unreleased]` will replace it.

### Added

- **Agent runtime** with Direct and Reactive (ReAct) execution modes, budget-aware execution (Normal / DeepThink / Ultra), mid-loop context compression, live context refresh, and cancellation tokens.
- **Cognitive memory layer** — semantic + episodic memory, FSRS5 spaced repetition, salience decay, knowledge-graph reflection, and a nightly Reforge self-improvement cycle.
- **Mirror subsystem** — event-driven self-reflection across six signal sources (routing, meta-rules, config archiving, trial preview, task focus, finance drift).
- **Skill system** with five built-in orchestrator skills (task management, finance, automation, learning, notebook) and progressive skill loading.
- **Storage** on SQLite (WAL) for relational data and LanceDB for vectors, both under `~/.klyntbot/`.
- **Desktop app** built on Tauri 2 + React 19, with secondary windows (launcher, tray, distraction overlay, voice orb), global hotkeys, and live tray countdown.
- **MCP server** (`klyntbot mcp serve --stdio`) exposing `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`, `annotate`, `learning`, `cron`, `mirror`, `temporal`.
- **Channel adapters** — Telegram, Discord, Slack, Email; all share the same memory and persona.
- **Feature packages** — tasks, finance, notes, productivity, coaching, insights, launcher, learning, language learning, activity log, notifications, plugin runtime, autotuner, voice engine, simulator.
- **Tiered History Compression (THC)** — turn-grouped, tier-assigned context-engine compression with extractive fallback.
- **Process hardening** at startup — `RLIMIT_CORE = 0`, `PT_DENY_ATTACH` on macOS, scrubbing of `LD_*`/`DYLD_*`/`MallocStackLogging*` env vars.
- **Dev/prod isolation** via `KLYNTBOT_HOME` and an auto-loaded `.env`.
- **Coding-memory ingestion** for Claude Code, Codex, Kimi CLI, and opencode (poll-only adapters).
- **AGPL-3.0** license.
- Initial open-source documentation: `README`, `CONTRIBUTING`, `SECURITY`, GitHub issue and PR templates.

### Known limitations

- macOS only (Apple Silicon and Intel). Linux and Windows are not supported.
- API keys in `~/.klyntbot/config.json` are stored in plaintext — full-disk encryption is the user's responsibility (see [SECURITY.md](./SECURITY.md)).
- CI workflows are not yet provided. Contributors must run the local check suite documented in [CONTRIBUTING.md](./CONTRIBUTING.md).
- Computer Use and procedural memory are designed but not implemented (see `docs/superpowers/specs/`).

[Unreleased]: https://github.com/KlyntLabs/klyntbot/commits/main
