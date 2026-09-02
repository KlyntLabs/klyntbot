# Project configuration (agent-facing)

Written by `configure-repo`. Skills read this file for repo-specific **machine config** —
commands, globs, paths — plus **posture** and **team** (below). Human-facing engineering
guidelines (coding standards, naming, house rules) live in `docs/product/guidelines.md`
when the project-docs layer is enabled; `plan-tasks` sources them from there and falls
back to this file otherwise.

## Project posture

The project's standing intent and lifecycle phase. Skills read this instead of re-asking:
`frame-change` and `clarify-decisions` right-size migration / compat / deprecation against
it; `interpret-session` reuses it. `clarify-decisions` production coverage needs Delivery
**Production** + Lifecycle **Cut Released / Scaling / Maintenance** + operate/launch
surface (or an ops ask); MVP, absent, and Early/Active stay off. Edit these two lines when
the project moves phase.

- **Delivery intent:** `Production` — how robust and complete the output must be.
- **Lifecycle stage:** `Active development` — where the project is in its life.
- **Catalog sync:** `full-triad` — triad dirs under `docs/specs/<slug>/` stay committed; map-features dispose-only (no materialize/export modes).
- **Default PR base:** `main` — `land-branch` reads it as the third rung of its base-resolution ladder.
- **Library docs:** Context7 MCP (preferred)

These are distinct from the product **Goals** in `docs/product/vision.md` (what success
looks like): posture is *how carefully to build right now*, not *what to build*.
## Team

Who works on this repo and how skills should package collaboration.
Skills that plan, review, or hand off read this section when present and
right-size **packaging** only (Solo / Small / Multi) — Iron Law gates never
change. Edit freely; re-run `/configure-repo` to re-draft from git/CODEOWNERS.
If this section is absent, skills do not invent a team.

**SSOT:** **band** derivation and the **packaging** matrix live only here.
Consumers **read** this section; they do not re-copy these rules into skill bodies.

### Roster

- Tech Lead — Jayden
- Contributor — Dalen Max

### Ownership notes (optional)

None

### Workflow band

- **Override (optional):**
- **Derive (when override blank):**
  1. Headcount from **Roster only**: each `Role — Name` = 1; each `N × Role` / `N Role(s)` adds N.
     Ignore Ownership notes and placeholders (`<…>`).
  2. Buckets: empty roster → **no band** (same packaging as Team absent); 1 → **Solo**;
     2–4 → **Small**; ≥5 → **Multi**.
  3. Specialty upgrade only: if **Small** and ≥3 distinct role titles (case-insensitive, trimmed),
     upgrade to **Multi**. Never downgrade Multi→Small.

### Packaging matrix

| Band | Packaging |
|---|---|
| **Solo** | Lean multi-person ritual language; no invented peer reviewers/assignees; agent-as-pair; full gates |
| **Small** | Design-review checkpoints; ownership boundaries via optional freeform notes; name people when roster has names |
| **Multi** | CODEOWNERS-aware review language when ownership notes exist; explicit review responsibilities as prose; write-handoff/docs emphasis |
| **(no band)** | Team absent, or empty roster with blank override — pre-feature default; do not invent a team; do not hard-fail |

## Decision boundaries

Optional. When present, `record-verdict` reads this table. Pins may raise a
floor or bind an action to a boundary type. An entry that would lower a core
floor is ignored with a one-line notice. Absent section → core table only.

| Action | Boundary-Type | Floor |
|---|---|---|
| | | |

## verify commands

Run in this order; all must pass before any completion claim.

| Check | Command |
|---|---|
| Typecheck | `cargo check --workspace` && `cd desktop-ui && bun run typecheck` |
| Lint | `cargo clippy --workspace --all-targets --all-features` && `cargo fmt --all --check` && `cd desktop-ui && bun run lint` |
| Unit tests | `cargo nextest run --workspace` |
| E2E / smoke | `./scripts/run_chat_perf_gates.sh` |

Single test file: Rust `cargo nextest run -E 'test(<fn_name>)'` (matches the test function name, e.g. `cargo nextest run -E 'test(advertised_equals_builtins)'`); UI `cd desktop-ui && bun run test -- <path>`The traceability check is not a command here — the `audit-trace` skill runs it as
`grep`/`git` over `docs/specs/` (and optional architecture). It is **docs-only**
and does not grep application tests for requirement IDs.

Legacy annotations in consumer code (if any) are ignored; do not require new ones.

Specs directory override:

## Run locally (dev)

How to start the app for user-facing acceptance checks (read by `validate-api`
and `validate-ui`). Fill in once the app can be run locally; leave a row blank
if that surface does not exist.

| Surface | Start command | Ready signal |
|---|---|---|
| Backend / API | `cargo tauri dev` (with Vite already up) — embedded HTTP on `:3456` | Dev HTTP / Tauri IPC responds |
| Frontend | `cd desktop-ui && bun run dev` | `http://localhost:1420` serves the app |

Browser E2E (Playwright, Chromium): `cd desktop-ui && bun run test:e2e`

## Remote environments

Read by `debug-remote` and `assess-observability`. Skip the table
(`None — not deployed`) when nothing is live. Never put tokens here.

None — not deployed

## release steps

Ordered list consumed by `cut-release`. GitHub release publish remains CI-owned
(`.github/workflows/release*.yml`) until you change that.

1. `cargo clippy --workspace --all-targets --all-features`
2. `cargo nextest run --workspace`
3. `cd desktop-ui && bun run build:check`
4. `cargo tauri build` (macOS bundle)
5. Smoke: `./scripts/run_chat_perf_gates.sh`

## Paths

- Specs: `docs/specs/`
- ADRs: `docs/adr/`
- Glossary: `CONTEXT.md`
- Out-of-scope KB: `.out-of-scope/`
- Engineering guidelines (project-docs layer): `docs/product/guidelines.md`
- Product vision / architecture spine (project-docs layer): `docs/product/vision.md`, `docs/architecture/`
