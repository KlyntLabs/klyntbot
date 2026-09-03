# Roadmap: KlyntBot

Status: Approved
Date: 2026-09-03

<!--
The program layer: milestone INTENT, above any single feature and below the product
vision. Optional — only multi-milestone projects need it. `plan-milestones` authors it;
`refresh-roadmap-status` derives progress from it without writing anything.

This file owns intent only: outcomes, ordering, membership, dependencies, commitments,
blockers, deferrals, goal dispositions. It never records how far a feature has got —
that lives once, as `Status:` in the feature's own requirements.md, mirrored into its
docs/specs/INDEX.md row. Every heading below is a REQUIRED slot — fill it or write
`None`.

Structural rules — AUTHORITATIVE. Both `plan-milestones` (before its approval gate) and
`refresh-roadmap-status` (as finding R11) validate against this list. A roadmap is structurally
defective when any of these does not hold:

  S1  every MILE-N and ROAD-N is defined exactly once
  S2  every ROAD-N sits under exactly one milestone
  S3  every milestone carries a non-empty Outcome sentence
  S4  no Depends-on names a milestone appearing later in the milestone table
  S5  every Depends-on resolves to exactly one live, non-struck-through MILE-N
  S6  every live GOAL-N in vision.md is cited by a milestone or listed under
      Goal dispositions
  S7  the milestone table and every milestone block parse

ID rules:
- Grammar: **MILE-<n>** and **ROAD-<n>**, flat and repo-wide.
- Stable from first definition. Retire by strikethrough with a reason
  (~~**MILE-3**~~ superseded by MILE-5) — never renumber, never reuse.
- A ROAD-N keeps its ID when it moves between milestones.
- Milestone ORDER is table row order; milestone IDENTITY is the MILE-N. Reordering the
  table never renumbers anything.
- ITEM order is list position within a milestone's Members, and it carries the build
  order: an item is buildable once the items above it are done. Same rule as milestones —
  order is position, identity is the ID, so resequencing members renumbers nothing.
- GOAL-N is defined in docs/product/vision.md and only cited here. Feature codes are
  defined in docs/specs/INDEX.md and never written here — a roadmap item is identified
  by its ROAD-N and slug until a feature spec binds to it.
- ROAD-N is a program SLOT (intent + order + Surfaces). A feature CODE is a separate
  delivery unit. Creating a ROAD does not create a feature. Binding is the INDEX
  "Roadmap item" cell (specify-behavior only). At most one live CODE binds a ROAD (R6).
  Same slug language does not make the IDs the same object.
-->

| ID | Milestone | Outcome | Depends-on | Commitment |
|---|---|---|---|---|
| MILE-1 | Coherent tool surface | The LLM, subagents, and MCP clients see one declared tool set, and the UI refreshes the right entities after a tool call. Work implemented and merged; closure pending an assessment. | none | Committed |
| MILE-2 | Baselines, verification, migration ledger | A contributor runs one verify entry point that reports frontend tests, token gates, Playwright WebKit proxy metrics, and repeatable production-runtime rendering evidence against recorded baselines, before any Lens rendering cost lands. | none | Committed |
| MILE-3 | Design-system foundation | The Lens token and recipe contract is live in both themes, and representative untouched semantic-token consumers inherit the new palette, type scale, and radii without blocking regressions. | MILE-2 | Planned |
| MILE-4 | Lens chat proof | Beside a first-party macOS 26 app, the chat window reads as the same design generation in both themes, with the ambient ground on by default only after native in-budget evidence. | MILE-3 | Planned |
| MILE-5 | Enforced migration | A contributor cannot land a legacy-styled screen: every hardened category reports zero unapproved violations across active-source scopes, and every page looks like one system. | MILE-4 | Planned |
| MILE-6 | HUD material migration | Launcher, tray, distraction overlay, and voice orb use the same Lens material vocabulary as the main window, on their own composition path, without double blur. | MILE-5 | Planned |
| MILE-7 | Chat information architecture | A user finds and manages conversations through the grouping and header actions decided for Lens, shipped separately from any visual migration. | MILE-6 | Planned |
| ~~MILE-8~~ | ~~Frontend long-tail cleanup~~ | retired 2026-09-03: defined prematurely; MILE-2's ledger must first classify the long tail and determine the correct milestone boundaries | — | — |

## MILE-1 — Coherent tool surface

**Outcome:** The LLM, subagents, and MCP clients see one declared tool set, and the UI refreshes the right entities after a tool call. Work implemented and merged; closure pending an assessment.
**Goals:** GOAL-1, GOAL-3
**Members:**
- **ROAD-1** tool-exposure-policy — Surfaces: `crates/tools-core/`, `crates/mcp/`, `crates/app-core/`, `desktop-ui/src/features/settings/`
- **ROAD-2** entity-update-intent — Surfaces: `crates/app-core/`, `crates/klyntbot-server/`
**Depends-on:** none
**Commitment:** Committed 2026-09-02
**Closed:** None
**Deferred:** None
**Blockers:** None

## MILE-2 — Baselines, verification, migration ledger

**Outcome:** A contributor runs one verify entry point that reports frontend tests, token gates, Playwright WebKit proxy metrics, and repeatable production-runtime rendering evidence against recorded baselines, before any Lens rendering cost lands.
**Goals:** GOAL-1
**Members:**
- **ROAD-3** frontend-verify-matrix — Surfaces: `docs/agents/project.md`, `desktop-ui/package.json`, `scripts/`
- **ROAD-4** rendering-proxy-lane — Surfaces: `desktop-ui/playwright.config.ts`, `desktop-ui/tests/`
- **ROAD-5** native-rendering-lane — Surfaces: None — the XCUIAutomation / XCTest / xctrace harness location is decided in its spec
- **ROAD-6** migration-ledger-and-central-exclusion — Surfaces: `scripts/check-design-tokens.sh`, `docs/standards/`
- **ROAD-7** backup-salvage-audit — Surfaces: `desktop-ui.new-bak/` (read-only), `docs/design/`
**Depends-on:** none
**Commitment:** Committed 2026-09-03
**Closed:** None
**Deferred:** None
**Blockers:** None

## MILE-3 — Design-system foundation

**Outcome:** The Lens token and recipe contract is live in both themes, and representative untouched semantic-token consumers inherit the new palette, type scale, and radii without blocking regressions.
**Goals:** GOAL-1
**Members:**
- **ROAD-8** lens-tokens — Surfaces: `packages/design-system/src/tokens/`
- **ROAD-9** lens-recipes-and-contract-tests — Surfaces: `packages/design-system/src/recipes/`, `packages/design-system/src/styles/theme.css`, `packages/design-system/src/lib/cn.ts`
- **ROAD-10** minimum-migration-scaffolding — Surfaces: `packages/design-system/src/recipes/legacy-glass.css`
- **ROAD-11** standards-and-docs-refresh — Surfaces: `docs/standards/`, `CLAUDE.md`, `desktop-ui/AGENTS.md`
**Depends-on:** MILE-2
**Commitment:** Planned
**Closed:** None
**Deferred:** None
**Blockers:** None

## MILE-4 — Lens chat proof

**Outcome:** Beside a first-party macOS 26 app, the chat window reads as the same design generation in both themes, with the ambient ground on by default only after native in-budget evidence.
**Goals:** GOAL-1
**Members:**
- **ROAD-12** lens-shell-chrome — Surfaces: `desktop-ui/src/app/layouts/`, `desktop-ui/src/features/chat/pages/ChatPage.tsx`, `desktop-ui/src/features/chat/components/ThreadList.tsx`, `desktop-ui/src/features/chat/components/ChatInput.tsx`
- **ROAD-13** ambient-ground-and-toggle — Surfaces: `desktop-ui/src/app/`, `desktop-ui/src/features/settings/`
- **ROAD-14** unified-message-rendering — Surfaces: `desktop-ui/src/features/chat/components/MessageList.tsx`, `desktop-ui/src/features/chat/components/VirtualizedMessageList.tsx`
- **ROAD-15** main-window-material-spike — Surfaces: `crates/desktop/tauri.conf.json`, `crates/desktop/src/lazy_window.rs`
**Depends-on:** MILE-3
**Commitment:** Planned
**Closed:** None
**Deferred:** None
**Blockers:** None

## MILE-5 — Enforced migration

**Outcome:** A contributor cannot land a legacy-styled screen: every hardened category reports zero unapproved violations across active-source scopes, and every page looks like one system.
**Goals:** GOAL-1
**Members:**
- **ROAD-16** glass-class-waves — Surfaces: `desktop-ui/src/features/`, `desktop-ui/src/shared/`, `desktop-ui/src/styles/`
- **ROAD-17** typography-weight-wave — Surfaces: `desktop-ui/src/`, `packages/design-system/src/primitives/`
- **ROAD-18** raw-color-and-arbitrary-size-wave — Surfaces: `desktop-ui/src/`
- **ROAD-19** gate-hardening — Surfaces: `scripts/check-design-tokens.sh`, `packages/design-system/src/recipes/`
**Depends-on:** MILE-4
**Commitment:** Planned
**Closed:** None
**Deferred:** None
**Blockers:** None

## MILE-6 — HUD material migration

**Outcome:** Launcher, tray, distraction overlay, and voice orb use the same Lens material vocabulary as the main window, on their own composition path, without double blur.
**Goals:** GOAL-1
**Members:**
- **ROAD-20** hud-window-material — Surfaces: `desktop-ui/src/features/launcher/`, `desktop-ui/src/features/tray/`, `desktop-ui/src/features/distraction/`, `desktop-ui/src/features/voice/`, `desktop-ui/src/shared/hooks/useTransparentBackground.ts`, `crates/desktop/src/lazy_window.rs`
**Depends-on:** MILE-5
**Commitment:** Planned
**Closed:** None
**Deferred:** None
**Blockers:** None

## MILE-7 — Chat information architecture

**Outcome:** A user finds and manages conversations through the grouping and header actions decided for Lens, shipped separately from any visual migration.
**Goals:** GOAL-1
**Members:**
- **ROAD-21** chat-grouping-and-header-actions — Surfaces: `desktop-ui/src/features/chat/components/ThreadList.tsx`, `desktop-ui/src/features/chat/pages/ChatPage.tsx`
**Depends-on:** MILE-6
**Commitment:** Planned
**Closed:** None
**Deferred:** None
**Blockers:** None

## ~~MILE-8~~ — Frontend long-tail cleanup

~~**MILE-8**~~ retired 2026-09-03: defined prematurely; MILE-2's ledger must first classify the long tail and determine the correct milestone boundaries. Its members are retired with it, never renumbered or reused:
- ~~**ROAD-22**~~ component-api-cleanup — retired 2026-09-03, same reason
- ~~**ROAD-23**~~ accessibility-beyond-shell — retired 2026-09-03, same reason
- ~~**ROAD-24**~~ dead-animation-and-theme-hooks — retired 2026-09-03, same reason

## Goal dispositions

Every live `GOAL-N` in `docs/product/vision.md` that no milestone cites belongs here, so
that a goal is never silently dropped (S6).

| Goal | Disposition | Date | Reason |
|---|---|---|---|
| GOAL-2 | Deferred | 2026-09-03 | Retained; outside this roadmap until a milestone genuinely delivers its local-first storage and explicit-approval outcome |
