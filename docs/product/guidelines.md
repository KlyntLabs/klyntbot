# Engineering Guidelines: KlyntBot

Status: Draft
Date: 2026-09-02

<!--
The human-facing engineering rules features must follow — coding standards,
conventions, house rules. Optional. When present, `plan-tasks` sources the
engineering rules for its Global Constraints from HERE rather than from
`docs/agents/project.md` (which then holds only machine-config + a pointer to this
file). Every heading is a REQUIRED slot — fill it or write `None`.

Seeded by configure-repo. Operational detail remains in CLAUDE.md; migrate or
expand with `/define-project` as needed.
-->

## Coding standards

- Rust: `common::Result<T>`; zero clippy warnings; conventional commits.
- Frontend: Bun only (never npm); path aliases `@/*`, `@shared/*`, `@features/*`, `@app/*`.
- Styling: design tokens only (`--ds-*`); no raw hex/rgb outside `packages/design-system/src/tokens/`.

## Naming and i18n

- Config serde: `camelCase`. Env overrides use double-underscore nesting.
- Timestamps: store UTC; display local via shared helpers (`formatTime()` on the UI).
- None additional for i18n yet.

## House rules

- Tests use `StoragePool::connect_in_memory()` — never `from_existing()` in tests.
- New Tauri commands use `#[klynt_command]` / `#[klynt_raw_command]`, not raw `#[tauri::command]` in the gated paths.
- AppCore handler methods: `#[tracing::instrument(skip(self), err)]`.
- Prefer editing `KLYNTBOT.md` / `KLYNTBOT-coding.md` for agent tone over hard-coding prompts in Rust.
