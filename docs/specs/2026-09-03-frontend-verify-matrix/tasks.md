# Tasks: Frontend verify matrix

> **For agentic workers:** after plan approval, pick one execute skill —
> `build-in-waves` (continuous dependency-aware scheduler), `build-by-story` (human-gated story review
> units), or `build-inline` (controller implements, no implementer subagents).
> The chosen skill writes `Execution-mode:`. Steps use checkbox (`- [ ]`) syntax
> for tracking.

Feature code: FVM
Status: In-progress
Date: 2026-09-03
Execution-mode: continuous
Max-concurrency: auto
Requirements: ./requirements.md
Design: ./design.md
Approved base (for guards): `3b3b1bab8d2ec80deef263df61960d89f2cfa40a` (merge-base with `main`)

**Goal:** One command, `bun run verify:frontend`, runs the six existing frontend checks from a manifest, prints one per-check summary with a single exit code, and is what the verify table and CI both call.

**Architecture:** A JSON manifest at `scripts/verify-frontend.manifest.json` lists checks (`name, command, cwd, mode, profiles`). A Bun-run TypeScript runner in `desktop-ui/scripts/verify-matrix/` loads it, selects by names or profile, spawns each check sequentially with piped stdout/stderr relayed live and retained, and prints a summary; the exit code is non-zero only after every selected check ran. A root `package.json` script is the entry; CI's `desktop-ui-quality` job calls it after installing chromium and ripgrep. Contract tests use a temporary manifest and fake commands; an acceptance script proves parity over the real six once.

**Tech Stack:** TypeScript on Bun (`node:child_process` `spawn`, `node:fs`, `node:path`), vitest 4 (per-file `// @vitest-environment node`), bash for the acceptance script, GitHub Actions on `macos-latest`.

## Global Constraints

Sources (path — sha256 prefix): `docs/agents/project.md` — `b8e0f8e7bcbda171` (verify commands, posture Production, Team band **Small**: Jayden tech lead, Dalen Max contributor); `docs/standards/desktop-ui.md` — `b2c26e47fd751fce` (scripts table, done gates); `docs/product/guidelines.md` — `0e7f3b4876e5cfc2` (Bun only, never npm; conventional commits; design tokens only). No `docs/architecture/` invariant applies (ARCH-1 concerns the desktop crate; no Rust or Tauri code here).

Compact delta for every task:
- Run tests with `cd desktop-ui && bun run test -- scripts/verify-matrix` (vitest picks up `desktop-ui/scripts/**/*.test.ts` by default). Test files start with `// @vitest-environment node`.
- Runner code imports only `node:*` builtins. No new dependency anywhere.
- **Do not edit** (guards FVM-5.1, 6.1–6.4): `desktop-ui/package.json`, `packages/design-system/src/tokens/`, `scripts/check-design-tokens.sh`, `desktop-ui/playwright.config.ts`, `desktop-ui/tests/`, `desktop-ui/scripts/check-performance-budget.sh`, and the `rust-quality` / `desktop-build-check` jobs in `.github/workflows/ci.yml`. Do not touch `desktop-ui.new-bak/`.
- Commit subjects: conventional (`feat(verify): …`, `ci(verify): …`, `docs(verify): …`); Co-Authored-By trailer per CLAUDE.md.
- Small band: name a reviewer only if the tech lead assigns one; no invented assignees.

## File Structure

| File | Responsibility |
|---|---|
| Create `scripts/verify-frontend.manifest.json` | Registry of checks: six initial entries, all `hard`, profiles `["default"]`, `cwd: "desktop-ui"`, run order = array order |
| Create `desktop-ui/scripts/verify-matrix/matrix.ts` | `loadManifest`, `selectChecks`, `splitCommand`, `runMatrix`, `formatSummary`, `ManifestError`, `SelectionError`, `ManifestEntry`, `Row` |
| Create `desktop-ui/scripts/verify-matrix/matrix.test.ts` | Contract tests for the four functions with temp manifests and fake `sh -c` commands |
| Create `desktop-ui/scripts/verify-matrix/main.ts` | CLI: argv → `loadManifest` → `selectChecks` → `runMatrix` → print → `process.exit` |
| Create `desktop-ui/scripts/verify-matrix/main.test.ts` | Integration tests spawning `bun main.ts` with `--manifest <temp>` |
| Create `desktop-ui/scripts/verify-matrix/acceptance.sh` | One-time parity and determinism run over the real six checks; records quick durations |
| Modify `package.json` (root) | Add script `verify:frontend` |
| Modify `.github/workflows/ci.yml` | `desktop-ui-quality` job: replace Lint/Type check/Test steps with ripgrep + chromium install + matrix run |
| Modify `docs/agents/project.md` | Verify table: Frontend row; Rust commands restructured but unchanged |
| Modify `docs/standards/desktop-ui.md` | New `## Verify matrix` section; Done gates point at the matrix |
| Modify `CLAUDE.md` | One line under `## Desktop UI (desktop-ui/)` naming `bun run verify:frontend` |

---

### Task 1: Manifest and loader

**Files:**
- Create: `scripts/verify-frontend.manifest.json`
- Create: `desktop-ui/scripts/verify-matrix/matrix.ts`
- Test: `desktop-ui/scripts/verify-matrix/matrix.test.ts`

**Reuse:** none — new code (rung 7); no registry of checks exists anywhere in the repo (design § Manifest).

**Interfaces:**
- Consumes: nothing.
- Produces: `type ManifestEntry = { name: string; command: string; cwd: string; mode: "hard" | "report"; profiles: string[] }`; `class ManifestError extends Error { path: string }`; `loadManifest(path: string): ManifestEntry[]`.

**Depends-on:** none

- [ ] **Step 1:** Failing tests in `matrix.test.ts`: `loadManifest` returns entries in file order for a valid temp manifest; throws `ManifestError` naming the path for a missing file, invalid JSON, an entry missing any of the five fields, and an entry with empty `profiles`. Run: `cd desktop-ui && bun run test -- scripts/verify-matrix` — expect: module not found.
- [ ] **Step 2:** Implement `ManifestEntry`, `ManifestError`, `loadManifest` (read with `node:fs`, `JSON.parse`, validate every field and `mode ∈ {hard, report}`). Run tests — expect: pass.
- [ ] **Step 3:** Write `scripts/verify-frontend.manifest.json`: entries `typecheck`, `lint`, `test`, `check:tokens`, `check:performance`, `test:e2e`, each `command: "bun run <name>"`, `cwd: "desktop-ui"`, `mode: "hard"`, `profiles: ["default"]`. Add a test that loads the real manifest (path resolved from `import.meta.dir` + `../../../scripts/`) and asserts exactly those six names in that order, all `hard`, all `default`. Run — expect: pass.
- [ ] **Step 4:** Commit: `feat(verify): add frontend check manifest and loader`.

_Requirements: FVM-1.6, FVM-2.1, FVM-2.5_

---

### Task 2: Check selection

**Files:**
- Modify: `desktop-ui/scripts/verify-matrix/matrix.ts`
- Test: `desktop-ui/scripts/verify-matrix/matrix.test.ts`

**Reuse:** none — new code (rung 7) (design § Runner core).

**Interfaces:**
- Consumes: `ManifestEntry` (Task 1).
- Produces: `class SelectionError extends Error { registeredNames: string[]; registeredProfiles: string[] }`; `selectChecks(entries: ManifestEntry[], sel: { names?: string[]; profile?: string }): ManifestEntry[]`.

**Depends-on:** Task 1

- [ ] **Step 1:** Failing tests: no selector → entries whose profiles include `default`, manifest order, and an entry with profiles `["nightly"]` is excluded; `names: ["b","a"]` → `[a, b]` in manifest order; `profile: "nightly"` → only that entry; unknown name or unknown profile → `SelectionError` whose message lists every registered name and profile. Run — expect: `selectChecks is not a function`.
- [ ] **Step 2:** Implement `selectChecks` and `SelectionError`. Run — expect: pass.
- [ ] **Step 3:** Commit: `feat(verify): select checks by name or profile`.

_Requirements: FVM-1.7, FVM-1.8, FVM-1.10, FVM-2.2_

---

### Task 3: Runner and summary

**Files:**
- Modify: `desktop-ui/scripts/verify-matrix/matrix.ts`
- Test: `desktop-ui/scripts/verify-matrix/matrix.test.ts`

**Reuse:** rung 3 — `node:child_process` `spawn` (Bun implements it; no dependency); rung 2 — the run-all-then-fail shape of `scripts/run_chat_perf_gates.sh` lines 129–134 as the reference pattern (design § Runner core).

**Interfaces:**
- Consumes: `ManifestEntry` (Task 1).
- Produces: `type Row = { name: string; mode: "hard" | "report"; result: "pass" | "fail"; exitStatus: number | null; durationSec: number; output: string; launchError?: string }`; `type Relay = { stdout: (chunk: string) => void; stderr: (chunk: string) => void }`; `splitCommand(command: string): string[]` (whitespace split; double-quoted segments kept whole; no shell operators supported); `runMatrix(entries: ManifestEntry[], opts: { repoRoot: string; spawn?: typeof import("node:child_process").spawn; relay?: Relay }): Promise<{ rows: Row[]; exitCode: 0 | 1 }>`; `formatSummary(rows: Row[]): string`.

**Depends-on:** Task 1

- [ ] **Step 1:** Failing tests with a temp manifest whose commands are `sh -c "echo ok"` (hard), `sh -c "echo boom >&2; exit 1"` (hard), `sh -c "echo findings; exit 1"` (report), and `definitely-not-a-command-xyz` (report), `cwd: "."`, `repoRoot` = the temp dir, `relay` collecting into buffers. Commands are split into argv by `splitCommand` (whitespace-separated, double-quoted segments kept whole) and spawned **without** a shell, so a missing executable raises the `error` event (ENOENT). Assert: all four rows present in order (hard failure did not stop the run); `exitCode === 1`; the report row with exit 1 has `result: "fail"` but would not on its own set exitCode (separate test with only that entry → `exitCode 0`); the unlaunchable entry has `result: "fail"`, `exitStatus: null`, `launchError` set, and alone yields `exitCode 1` despite `report`; `rows[2].output` contains `findings` and the relay buffer contains `findings` and `boom`; `durationSec >= 0`; each row's `exitStatus` equals the fake command's exit code; two consecutive runs produce identical `formatSummary` text. Run — expect: `runMatrix is not a function`.
- [ ] **Step 2:** Implement `splitCommand` and `runMatrix` (sequential `await` per entry; `const [cmd, ...args] = splitCommand(entry.command); spawn(cmd, args, { cwd: join(repoRoot, entry.cwd), stdio: ["inherit","pipe","pipe"] })` — no shell; on `data` write through to relay and append to `output`; on `error` record `launchError`; print `▶ <name>  (<mode>)` before each check) and `formatSummary` (columns name · mode · result · exit · seconds, aligned, one row per check, one header line). Run — expect: pass.
- [ ] **Step 3:** Commit: `feat(verify): run checks sequentially with relayed output and one summary`.

_Requirements: FVM-1.2, FVM-1.3, FVM-1.4, FVM-1.5, FVM-2.4, FVM-7.2_

---

### Task 4: CLI entry and root script

**Files:**
- Create: `desktop-ui/scripts/verify-matrix/main.ts`
- Modify: `package.json` (root) — `scripts` block
- Test: `desktop-ui/scripts/verify-matrix/main.test.ts`

**Reuse:** rung 6 — one root `package.json` script line; rung 7 for `main.ts` (argv parsing is ~30 lines, no dependency justified) (design § CLI entry).

**Interfaces:**
- Consumes: `loadManifest`, `selectChecks`, `runMatrix`, `formatSummary`, `ManifestError`, `SelectionError` (Tasks 1–3).
- Produces: command `bun run verify:frontend [check …] [--profile <name>] [--list] [--manifest <path>]`; root script `"verify:frontend": "bun desktop-ui/scripts/verify-matrix/main.ts"`; default manifest path `join(import.meta.dir, "../../../scripts/verify-frontend.manifest.json")`.

**Depends-on:** Task 1, Task 2, Task 3

- [ ] **Step 1:** Failing integration tests in `main.test.ts` that `spawnSync("bun", [mainPath, ...args])` with `--manifest <temp>`: `--list` prints every entry's name, mode, profiles, cwd, command and exits 0 without running (a temp entry whose command writes a marker file; marker absent afterwards); `--list` against the real manifest prints the six default names; an appended temp entry with profile `["extra"]` runs under `--profile extra` (marker present) and not under no-arg (marker absent); unknown name → exit 1 and stderr lists registered names; unparsable manifest → exit 1 and stderr names the path. Run — expect: main.ts not found.
- [ ] **Step 2:** Implement `main.ts`: parse argv (positional names, `--profile`, `--list`, `--manifest`), wrap `ManifestError`/`SelectionError` → print message, exit 1; print summary; `process.exit(exitCode)`. Run — expect: pass.
- [ ] **Step 3:** Add `"verify:frontend": "bun desktop-ui/scripts/verify-matrix/main.ts"` to root `package.json` scripts. Run `bun run verify:frontend --list` from the repo root — expect: six rows, exit 0.
- [ ] **Step 4:** Commit: `feat(verify): add bun run verify:frontend entry point`.

_Requirements: FVM-1.1, FVM-1.9, FVM-2.3_

---

### Task 5: CI calls the matrix

**Files:**
- Modify: `.github/workflows/ci.yml` — `desktop-ui-quality` job only (lines 50–74 at the approved base)

**Reuse:** rung 2 — extend the existing `desktop-ui-quality` job; rung 4 — `oven-sh/setup-bun`, Homebrew, and `playwright install` already available on the runner (design § CI job).

**Interfaces:**
- Consumes: `bun run verify:frontend` (Task 4).
- Produces: job steps after `Install dependencies`: `Install ripgrep` (`command -v rg || brew install ripgrep`), `Install Playwright browser` (`working-directory: desktop-ui`, `bunx playwright install chromium`), `Verify frontend` (`bun run verify:frontend` at repo root). The `Lint`, `Type check`, `Test` steps are removed. Job name unchanged.

**Depends-on:** Task 4

- [ ] **Step 1:** Edit the job as specified. Run `git diff --exit-code <approved-base> -- .github/workflows/ci.yml` piped through `sed -n '14,48p;76,151p'` on both versions (extract the two other job blocks from base and working tree and `diff` them) — expect: no difference.
- [ ] **Step 2:** Validate YAML locally: `python3 -c 'import yaml;print(list(yaml.safe_load(open(".github/workflows/ci.yml"))["jobs"]))'` (PyYAML is installed) — expect: `['rust-quality', 'desktop-ui-quality', 'desktop-build-check']`.
- [ ] **Step 3:** Commit: `ci(verify): run the frontend verify matrix in desktop-ui-quality`. Push the branch and confirm the job runs the six checks and passes; note the job's wall-clock duration in the task report (FVM-4.4).

_Requirements: FVM-4.1, FVM-4.2, FVM-4.3, FVM-4.4, FVM-4.5_

---

### Task 6: Docs name the matrix

**Files:**
- Modify: `docs/agents/project.md` — `## verify commands` table (lines 76–85 at the approved base)
- Modify: `docs/standards/desktop-ui.md` — new `## Verify matrix` section between `## Scripts` and `## Path aliases`; `## Done gates` paragraph
- Modify: `CLAUDE.md` — `## Desktop UI (desktop-ui/)` command block (lines 30–42 at the approved base)

**Reuse:** rung 2 — extend the existing verify table and standards doc (design § Docs).

**Interfaces:**
- Consumes: `bun run verify:frontend`, `--list` (Task 4).
- Produces: project.md rows `| Typecheck (Rust) | cargo check --workspace |`, `| Lint (Rust) | cargo clippy … && cargo fmt --all --check |`, `| Unit tests (Rust) | cargo nextest run --workspace |`, `| Frontend | bun run verify:frontend |` (note: `bun run verify:frontend --list` shows the registered checks), `| E2E / smoke | ./scripts/run_chat_perf_gates.sh |`.

**Depends-on:** Task 4

Neighbor note (advisory from retrieval): the EXPO and EUPI plans pin `CLAUDE.md` (both) and `docs/agents/project.md` (EUPI) by sha256 as their Global Constraints source; editing these files leaves those pins stale. Informational only, no action in this feature.

- [ ] **Step 1:** Rewrite the verify table as above; assert with `grep -c` that `cargo check --workspace`, `cargo clippy --workspace --all-targets --all-features`, `cargo fmt --all --check`, `cargo nextest run --workspace`, and `./scripts/run_chat_perf_gates.sh` each still appear exactly once.
- [ ] **Step 2:** Add `## Verify matrix` to desktop-ui.md: entry command, manifest path and five fields, `hard` vs `report`, profiles (`default` only today), `--list` as discovery only, where ROAD-4/ROAD-5 lanes register. Update `## Done gates` to name `bun run verify:frontend` as the completion-claim command.
- [ ] **Step 3:** Add `bun run verify:frontend   # all six frontend checks, one summary (run from repo root)` to CLAUDE.md's Desktop UI block.
- [ ] **Step 4:** Commit: `docs(verify): add the frontend verify matrix to the verify table and standards`.

_Requirements: FVM-3.1, FVM-3.2, FVM-3.3_

---

### Task 7: Acceptance run and change-boundary proof

**Files:**
- Create: `desktop-ui/scripts/verify-matrix/acceptance.sh`

**Reuse:** rung 2 — the hand-rolled bash test pattern of `scripts/adapt_codex_vendor.sh.test.sh` (`set -euo pipefail`, run, assert with `test` / `grep`) (design § Acceptance script).

**Interfaces:**
- Consumes: `bun run verify:frontend` and the six package scripts.
- Produces: a printed acceptance report: per-check exit parity, twice-run summary diff, quick-subset durations, and the boundary diff results.

**Depends-on:** Task 4

- [ ] **Step 1:** Write `acceptance.sh` (run from repo root): for each of the six names run `(cd desktop-ui && bun run <name>)` and `bun run verify:frontend <name>`, capturing exit codes, and assert equal (FVM-5.2); run `bun run verify:frontend` twice, extract the summary block from each, `diff` them (FVM-7.3); parse the four quick-check durations from the summary and assert their sum ≤ 120 (FVM-7.1); grep the relayed output for the token gate's unconditional `==> Raw color literals` section header (the `(soft)` note appears only when findings exist), the Playwright `1 passed` line, and the perf budget's `gzip` column header (FVM-5.3, 5.4, 5.5).
- [ ] **Step 2:** Boundary diffs, all against `3b3b1bab8d2ec80deef263df61960d89f2cfa40a`: `git diff --exit-code <base> -- packages/design-system/src/tokens/ scripts/check-design-tokens.sh desktop-ui/playwright.config.ts desktop-ui/tests/ desktop-ui/scripts/check-performance-budget.sh` (FVM-6.1–6.3); `git diff --exit-code <base> -- desktop-ui/package.json` (FVM-5.1); the two Rust job blocks as in Task 5 Step 1 (FVM-6.4). Include these in `acceptance.sh` so the run prints them.
- [ ] **Step 3:** Run `./desktop-ui/scripts/verify-matrix/acceptance.sh` on the finished branch with no stray dev server on port 1420 — expect: every assertion passes; paste the report into the task report.
- [ ] **Step 4:** Commit: `test(verify): add one-time acceptance script for parity, determinism, and change boundary`.

_Requirements: FVM-5.1, FVM-5.2, FVM-5.3, FVM-5.4, FVM-5.5, FVM-6.1, FVM-6.2, FVM-6.3, FVM-6.4, FVM-7.1, FVM-7.3_
