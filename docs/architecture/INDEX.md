# Architecture: KlyntBot

Status: Draft
Date: 2026-09-02

<!--
The architecture spine: the small set of INVARIANTS that keep independently-built
features consistent — not a diagram doc. Optional. Each invariant is a bold
**ARCH-N** ID plus ONE imperative sentence (the rule). Feature design.md files cite
the ones they rely on as `Respects: ARCH-N`, and the `audit-trace` check verifies those
citations point at a live invariant.

Rules:
- ID grammar: **ARCH-<n>**, flat and repo-wide (unique forever, never reuse).
- One rule per invariant. If it needs "and", it is usually two invariants.
- IDs are immutable once relied upon. Retire by strikethrough, never renumber.
- Large project? Split invariants into per-domain files (`docs/architecture/<domain>.md`)
  and list them under "Domains" below; the ARCH-N namespace stays flat across files.

Seeded by configure-repo. Replace placeholders with real invariants via `/define-project`.
CLAUDE.md may still reference deeper architecture narrative docs — author those under
this directory as the spine grows.
-->

## Invariants

- **ARCH-1** Shared business logic lives in `app-core`; the `desktop` crate remains a thin Tauri adapter.
- **ARCH-2** Session mode (`assistant` | `coding`) is set at creation and is immutable.
- **ARCH-3** Every tool call passes through the approval gate with a declared approval class.

## Domains

None yet (all invariants live above). Split into per-domain files when the spine grows.
