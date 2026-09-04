# Tasks: Rendering proxy lane

> **For agentic workers:** after plan approval, pick one execute skill —
> `build-in-waves` (continuous dependency-aware scheduler), `build-by-story` (human-gated story review
> units), or `build-inline` (controller implements, no implementer subagents).
> The chosen skill writes `Execution-mode:`. Steps use checkbox (`- [ ]`) syntax
> for tracking.

Feature code: PERFPROXY
Status: Approved
Date: 2026-09-04
Execution-mode: unset
Max-concurrency: auto
Requirements: ./requirements.md
Design: ./design.md
Approved base (for guards): `c3c50f052202f062fa889973c44b5dabe3f8e704` (main at plan approval). Build order: after ROAD-25 lands (its lint fix `b4c54c29d` is already on main).

**Goal:** `bun run perf:proxy` measures six chat rows in headless WebKit against a committed baseline and ends in `HEALTHY`, `DEGRADED`, or `COULD_NOT_MEASURE` with a matching exit code, locally, through the verify matrix, and in a path-filtered CI workflow.

**Architecture:** Playwright specs (`tests/perf-proxy/rows.perf.ts`, own config, one WebKit project, serial) mock the chat page's nine IPC commands, prove the fixture and rendering path, sample rAF gaps, time K screenshots, and write one row file each. A Bun-run wrapper (`scripts/perf-proxy/run.ts`) starts clean, spawns Playwright, merges rows, derives the environment identity, compares against `tests/perf-proxy/baselines/<environmentKey>.json`, and finalizes outcome, `latest.json`, step summary, and exit code through one path. Recalibration is a separate entry point. A dedicated workflow runs the lane on relevant paths and uploads the envelope.

**Tech Stack:** TypeScript on Bun (wrapper) and Node (Playwright child), `@playwright/test` 1.62 WebKit, vitest 4 (`// @vitest-environment node`), `tsc` with `tsconfig.perf.json` + `@types/node`, GitHub Actions on `macos-latest`.

## Global Constraints

Sources (path — sha256 prefix at plan time): `docs/agents/project.md` — `671baf0fbadb5651`; `docs/standards/desktop-ui.md` — `9cffa97268b67154`; `docs/standards/frontend-performance.md` — `6e98b27763f5d0e8`; `docs/product/guidelines.md` — `0e7f3b4876e5cfc2`. Team band **Small** (Jayden tech lead, Dalen Max contributor). No `docs/architecture/` invariant applies.

Compact delta for every task:
- Unit tests: `cd desktop-ui && bun run test -- scripts/perf-proxy`; every vitest file lives under `desktop-ui/scripts/perf-proxy/` (vitest excludes `tests/**`, and any `*.test.ts` under `tests/` would be loaded by the chromium smoke config); files start with `// @vitest-environment node`. Type-check: `cd desktop-ui && bun run perf:proxy:typecheck` once Task 2 lands.
- Imports of shared types are relative (`../../src/shared/types/chat`); the `@shared/*` alias does not resolve here.
- **Do not edit** (PERFPROXY-5.7, 5.8): `desktop-ui/playwright.config.ts`, the `test:e2e` script, `desktop-ui/tests/smoke/`, `desktop-ui/scripts/verify-matrix/`, the six existing manifest entries. Do not touch `desktop-ui.new-bak/`.
- No fixture mode or transport branch in `desktop-ui/src`; the only product-source change is one `data-render-path` attribute per list root.
- Neighbor owner **FVM** for `scripts/verify-frontend.manifest.json`, `docs/agents/project.md`, `docs/standards/desktop-ui.md` (FVM hash-pins the latter two): edits are additive at the named anchors only, guarded by diff.
- Commit subjects conventional (`feat(perf-proxy): …`, `ci(perf-proxy): …`, `docs(perf-proxy): …`); Co-Authored-By trailer per CLAUDE.md.

## File Structure

| File | Responsibility |
|---|---|
| Create `desktop-ui/scripts/perf-proxy/contract.ts` | Versions, R/K constants, margin policy, `Identity`, `RowResult`, `LatestRun`, `Baseline`, `Outcome`, `Subcode` types |
| Create `desktop-ui/scripts/perf-proxy/env.ts` | `describeEnvironment` (identity from env + os + row environments; hostname digest only) |
| Create `desktop-ui/scripts/perf-proxy/compare.ts` | `identityMatches`, `compareRows` (pure) |
| Create `desktop-ui/scripts/perf-proxy/summary.ts` | `renderSummary`, `writeStepSummary` |
| Create `desktop-ui/scripts/perf-proxy/run.ts` | Routine wrapper: clean start, spawn Playwright, merge, decide, `finalizeOutcome`; `--list` passthrough |
| Create `desktop-ui/scripts/perf-proxy/recalibrate.ts` | R repetitions → baseline (tracked file or `--candidate <dir>`) |
| Create `desktop-ui/scripts/perf-proxy/*.test.ts` | Contract tests for the five modules above |
| Create `desktop-ui/tests/perf-proxy/support/rowfile.ts` | `ROWS_DIR`, `EXPECTED_ROWS`, `writeRowFile`, `readRowFiles` |
| Create `desktop-ui/tests/perf-proxy/support/fixtures.ts` | `buildMessages`, `buildThread`, `ALLOWLIST`, `installMocks` |
| Create `desktop-ui/scripts/perf-proxy/fixtures.test.ts` | Fixture shape tests (vitest, node env); lives under `scripts/` because vitest excludes `tests/**` and Playwright's default config would treat a `*.test.ts` under `tests/` as a spec |
| Create `desktop-ui/tests/perf-proxy/support/preconditions.ts` | `setTheme`, `expectTheme`, `expectPlainRendered`, `expectVirtualizedRendered` |
| Create `desktop-ui/tests/perf-proxy/support/sampler.ts` | `sampleRaf`, `captureScreenshots`, `stats`, `SCREENSHOT_OPTIONS` |
| Create `desktop-ui/tests/perf-proxy/rows.perf.ts` | Six generated row tests |
| Create `desktop-ui/tests/perf-proxy/baselines/` | Committed baselines (first local one lands in Task 5; CI one in Task 8) |
| Create `desktop-ui/playwright.perf-proxy.config.ts` | Perf config: WebKit project, serial, `*.perf.ts`, own webServer |
| Create `desktop-ui/tsconfig.perf.json` | Type-check for `tests/perf-proxy/**` and `scripts/perf-proxy/**` |
| Modify `desktop-ui/package.json` | Scripts `perf:proxy`, `perf:proxy:recalibrate`, `perf:proxy:typecheck`; devDependency `@types/node` |
| Modify `bun.lock` (root) | Workspace lockfile for `@types/node`; `desktop-ui/bun.lock` is a stale pre-workspace file and must stay unchanged |
| Modify `desktop-ui/src/features/chat/components/MessageList.tsx` | `data-render-path="plain"` on the root at line 135 |
| Modify `desktop-ui/src/features/chat/components/VirtualizedMessageList.tsx` | `data-render-path="virtualized"` on the root at line 336 |
| Modify `scripts/verify-frontend.manifest.json` | Append the `perf-proxy` entry (FVM-owned; six entries byte-identical) |
| Create `.github/workflows/rendering-proxy.yml` | Path-filtered routine job + dispatch recalibration; artifacts |
| Modify `docs/standards/frontend-performance.md` | `## Rendering proxy lane (advisory)` after line 61 |
| Modify `docs/standards/desktop-ui.md` | Extend `## Verify matrix` (lines 32–51; FVM-owned anchor) |
| Modify `docs/agents/project.md` | Line 109 browser-E2E wording (FVM-owned anchor) |

---

### Task 1: Contract, identity, and comparison

**Files:**
- Create: `desktop-ui/scripts/perf-proxy/contract.ts`, `desktop-ui/scripts/perf-proxy/env.ts`, `desktop-ui/scripts/perf-proxy/compare.ts`
- Test: `desktop-ui/scripts/perf-proxy/env.test.ts`, `desktop-ui/scripts/perf-proxy/compare.test.ts`

**Reuse:** rung 3 — `node:os` (`platform`, `arch`, `release`, `hostname`, `cpus`), `node:crypto` `createHash`; rung 4 — `browser.version()` passed in from the spec via the row file's `environment` block; rung 7 — the identity derivation, margin policy, and comparison logic are new code (design § Contract, identity, and comparison).

**Interfaces:**
- Consumes: nothing.
- Produces: `SCHEMA_VERSION = 1`, `MEASUREMENT_CONTRACT_VERSION = 1`, `R_REPETITIONS = 5`, `K_CAPTURES = 10`, `MARGIN_POLICY = { rafP95: { multiplier: 3, floorMs: 2 }, screenshotP50: { multiplier: 3, floorRatio: 0.15 } }` (candidates; Task 8 confirms); `type Identity = { schemaVersion; measurementContractVersion; environmentKey }`; `type RowEnvironment = { webkitVersion; headless; viewport: { width; height }; dpr; userAgent }`; `type RowResult = { row; scenario; theme; n; renderPath; raf: { sampleCount; p50Ms; p95Ms; maxMs }; screenshot: { captureCount; p50Ms; options }; environment: RowEnvironment; durationMs }`; `type MetricStat = { median; spread; margin }`; `type Baseline = { identity; recordedAt; sourceRevision; repetitions; rows: Record<string, { raf: { p95: MetricStat }; screenshot: { p50: MetricStat } }>; environment: { keyed; diagnostic } }`; `type Outcome = "HEALTHY" | "DEGRADED" | "COULD_NOT_MEASURE"`; `type Subcode = "NO_BASELINE" | "PRECONDITION" | "UNEXPECTED_COMMAND" | "LAUNCH" | "TIMEOUT" | "IDENTITY" | "ROW_MISSING" | "LATEST_MALFORMED" | "CHILD_FAILED" | "PORT_BUSY" | "WRITE_FAILED"`; `type LatestRun = { identity; environment; rows: RowResult[]; outcome; subcode?; comparison?: RowDelta[]; recordedAt }`; `describeEnvironment({ env, os, rows }): { key; keyed; diagnostic }` throwing `IdentityMismatch`; `identityMatches(a, b)`; `compareRows(latest, baseline): { outcome: "HEALTHY" | "DEGRADED"; rows: RowDelta[] }` with `RowDelta = { row; metric; baseline; current; delta; margin; exceeded }`.

**Depends-on:** none

- [ ] **Step 1:** Failing tests: `describeEnvironment` yields `runnerClass` `github:<ImageOS>` when `env.CI` and `env.ImageOS` are set, else `local:<64-hex sha256(hostname)>`; the returned key is 16 hex; the same inputs give the same key; changing any keyed dimension changes the key; changing CPU model does not; rows with differing `environment` throw `IdentityMismatch`; `JSON.stringify` of the result never contains the raw hostname. Run: `cd desktop-ui && bun run test -- scripts/perf-proxy` — expect: module not found.
- [ ] **Step 2:** Failing tests: `compareRows` returns `HEALTHY` when every metric is within its stored margin, `DEGRADED` naming row/metric/baseline/current/delta when one exceeds, and treats a metric exactly at the margin as within; `identityMatches` is false when any of the three fields differ.
- [ ] **Step 3:** Implement `contract.ts`, `env.ts`, `compare.ts`. Run — expect: pass.
- [ ] **Step 4:** Commit: `feat(perf-proxy): add measurement contract, environment identity, and comparison`.

_Requirements: PERFPROXY-2.1, PERFPROXY-2.2, PERFPROXY-2.3, PERFPROXY-3.5_

---

### Task 2: Fixtures, row files, and the type-check

**Files:**
- Create: `desktop-ui/tests/perf-proxy/support/fixtures.ts`, `desktop-ui/tests/perf-proxy/support/rowfile.ts`, `desktop-ui/tsconfig.perf.json`
- Modify: `desktop-ui/package.json` (add `@types/node` devDependency; add script `perf:proxy:typecheck`), `bun.lock` (root)
- Test: `desktop-ui/scripts/perf-proxy/fixtures.test.ts` (imports `../../tests/perf-proxy/support/fixtures` and `.../rowfile`)

**Reuse:** rung 2 — `ChatMessage` and `ChatThread` from `desktop-ui/src/shared/types/chat.ts` (relative import), `AppInfoResponse` from `src/shared/types/common.ts`; rung 4 — `page.route` + `route.fulfill({ json })`, `page.addInitScript` (design § Fixtures); rung 3 — `node:fs` (design § Row file I/O); rung 2 — `tsconfig.node.json` as the shape; rung 4 — `tsc --noEmit -p`; rung 5 — `@types/node` (design § Type-check config).

**Interfaces:**
- Consumes: `RowResult` (Task 1).
- Produces: `buildMessages(n): ChatMessage[]` (ids `m1…mn`, alternating user/assistant, timestamps from epoch `1756800000000` + 60 s × i, content `"msg <i> — " + filler`); `buildThread(sessionKey, n): ChatThread`; `SESSION_KEY = "perf-proxy-session"`; `ALLOWLIST: Record<string, (n: number) => unknown>` for the nine commands; `installMocks(page, { n }): { unexpected: string[] }` (unknown commands → status 500 + recorded; `EventSource` stubbed); `ROWS_DIR`, `EXPECTED_ROWS` (six names `idle-20×light` … `scroll-200×dark`), `writeRowFile`, `readRowFiles`; script `perf:proxy:typecheck` = `tsc --noEmit -p tsconfig.perf.json`.

**Depends-on:** Task 1

- [ ] **Step 1:** Failing tests: `buildMessages(20)` has 20 entries with unique ids, alternating roles, strictly increasing timestamps, no markdown characters; `ALLOWLIST` has exactly the nine command names; `writeRowFile` then `readRowFiles` round-trips a `RowResult` in a temp dir; `readRowFiles` on an empty dir returns `[]`. Run: `cd desktop-ui && bun run test -- scripts/perf-proxy` — expect: module not found.
- [ ] **Step 2:** `cd desktop-ui && bun add -d @types/node`, then `git status --short` — expect: root `bun.lock` and `desktop-ui/package.json` changed, `desktop-ui/bun.lock` untouched; write `tsconfig.perf.json` (`strict`, `noEmit`, `skipLibCheck`, `module ESNext`, `moduleResolution bundler`, `target ES2022`, `lib ["ES2023","DOM"]`, `types ["node"]`, include `tests/perf-proxy/**/*.ts`, `scripts/perf-proxy/**/*.ts`); add the script. Implement the modules. Run tests — expect: pass; run `bun run perf:proxy:typecheck` — expect: exit 0; `bun run test:e2e -- --list` — expect: still only the chromium smoke.
- [ ] **Step 3:** Commit: `feat(perf-proxy): add typed chat fixtures, row files, and the perf type-check`.

_Requirements: PERFPROXY-1.3, PERFPROXY-1.11_

---

### Task 3: Six rows measured in WebKit

**Files:**
- Create: `desktop-ui/playwright.perf-proxy.config.ts`, `desktop-ui/tests/perf-proxy/support/preconditions.ts`, `desktop-ui/tests/perf-proxy/support/sampler.ts`, `desktop-ui/tests/perf-proxy/rows.perf.ts`
- Modify: `desktop-ui/src/features/chat/components/MessageList.tsx` (root at line 135), `desktop-ui/src/features/chat/components/VirtualizedMessageList.tsx` (root at line 336)

**Reuse:** rung 2 — `desktop-ui/playwright.config.ts` as the shape to mirror; rung 4 — Playwright config fields `testDir`, `testMatch`, `outputDir`, `projects`, `webServer` (design § Perf Playwright config); rung 2 — `ThemeProvider.tsx:20/34`; rung 4 — `page.addInitScript`, `page.waitForFunction`, `locator.waitFor` (design § Preconditions); rung 2 — the spike's sampler shape (`.skills/PERFPROXY/spike-repeatability.md`) reimplemented under test-first; rung 4 — `requestAnimationFrame`, `performance.now`, `page.evaluate`, `page.screenshot`; rung 7 — the percentile statistics and scroll driver are new code (design § In-page sampler); rung 2 — `tests/smoke/theme-toggle.spec.ts` as the spec shape; rung 4 — `test.describe.configure({ mode: "serial" })` (design § Row specs).

**Interfaces:**
- Consumes: `installMocks`, `buildMessages`, `SESSION_KEY`, `writeRowFile`, `EXPECTED_ROWS` (Task 2); `RowResult`, `K_CAPTURES` (Task 1).
- Produces: config per design (project `webkit` with `{ ...devices["Desktop Safari"], viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 }`, `outputDir: "test-results/perf-proxy/artifacts"`, `reuseExistingServer: false`, `trace: "off"`); `setTheme(context, theme)`, `expectTheme(page, theme)`, `expectPlainRendered(page, messages)`, `expectVirtualizedRendered(page, messages)`; `sampleRaf(page, { warmupFrames: 60, sampleFrames: 300, scroll?: { stepPx: 120 } }): Promise<number[]>`, `captureScreenshots(page, k): Promise<number[]>`, `stats(values): { p50; p95; max }`, `SCREENSHOT_OPTIONS = { type: "png", scale: "css", animations: "allow", caret: "hide", fullPage: false }`; `ROWS` table; attribute `data-render-path`.

**Depends-on:** Task 2

- [ ] **Step 1:** Add `data-render-path="plain"` to the `MessageList` root and `"virtualized"` to the `VirtualizedMessageList` root; `cd desktop-ui && bun run test` — expect: existing suite still green.
- [ ] **Step 2:** Write the config and `rows.perf.ts` with a failing first row (no support yet). Run `cd desktop-ui && bunx playwright test --config playwright.perf-proxy.config.ts` — expect: failure naming the missing support module; `bunx playwright test --config playwright.perf-proxy.config.ts --list` — expect: six rows, project `webkit`; `bun run test:e2e -- --list` — expect: only the chromium smoke.
- [ ] **Step 3:** Implement `preconditions.ts` and `sampler.ts`; each row: context with `reducedMotion: "no-preference"` and the theme init script → `installMocks` → `goto("/#/chat?thread=" + SESSION_KEY)` → `expectTheme` → path precondition and `unexpected` empty → 600 ms → `sampleRaf` → 600 ms → `captureScreenshots(K_CAPTURES)` → `writeRowFile`. Run the config — expect: six passes, six files under `test-results/perf-proxy/rows/`, each with the `environment` block.
- [ ] **Step 4:** Negative checks: temporarily answer `journey_milestones` with a 500-only allowlist edit and confirm the row fails naming the command; revert. Confirm the run's own wall-clock (printed by Playwright) is under 60 s excluding launch.
- [ ] **Step 5:** Commit: `feat(perf-proxy): measure six chat rows in headless WebKit`.

_Requirements: PERFPROXY-1.1, PERFPROXY-1.2, PERFPROXY-1.4, PERFPROXY-1.5, PERFPROXY-1.6, PERFPROXY-1.7, PERFPROXY-1.8, PERFPROXY-1.9, PERFPROXY-1.10, PERFPROXY-5.4, PERFPROXY-5.5, PERFPROXY-5.6, PERFPROXY-7.1_

---

### Task 4: Wrapper with one finalization path, registered in the matrix

**Files:**
- Create: `desktop-ui/scripts/perf-proxy/run.ts`, `desktop-ui/scripts/perf-proxy/summary.ts`
- Modify: `desktop-ui/package.json` (script `perf:proxy` = `bun scripts/perf-proxy/run.ts`), `scripts/verify-frontend.manifest.json` (append entry)
- Test: `desktop-ui/scripts/perf-proxy/run.test.ts`, `desktop-ui/scripts/perf-proxy/summary.test.ts`

**Reuse:** rung 2 — FVM's `main.ts` argv shape and its `spawn` injection pattern (`desktop-ui/scripts/verify-matrix/main.ts:19-48`, `matrix.ts:187-248`), FVM's `main.test.ts` spawn-in-test pattern; rung 3 — `node:child_process` `spawn`, `node:fs`; rung 7 — the outcome state machine and subcodes are new code (design § Wrapper CLI); rung 3 — `node:fs` `appendFileSync` (design § Step summary).

**Interfaces:**
- Consumes: Task 1 types and functions; `ROWS_DIR`, `EXPECTED_ROWS`, `readRowFiles` (Task 2); the perf config (Task 3).
- Produces: `runProxy(opts: { spawn?; rowsDir?; baselinesDir?; latestPath?; env? }): Promise<{ outcome; subcode?; exitCode }>`; `finalizeOutcome(input): Promise<0 | 1 | 2>` (atomic `latest.json` via temp + rename, print, summary); `renderSummary(run, comparison?, subcode?)`, `writeStepSummary(markdown, env)`; manifest entry `{ "name": "perf-proxy", "command": "bun run perf:proxy", "cwd": "desktop-ui", "mode": "report", "profiles": ["perf-proxy"] }`.

**Depends-on:** Task 1, Task 2, Task 3

- [ ] **Step 1:** Failing consistency table in `run.test.ts` (temp dirs, injected `spawn` returning a fake child): for each terminal path — child exit 0 + six rows + matching baseline within margins → `HEALTHY` 0; one metric over → `DEGRADED` 1; no baseline → `NO_BASELINE` 2; child exit 1 → `CHILD_FAILED` 2; child stderr containing `is already used` → `PORT_BUSY` 2; a missing row → `ROW_MISSING` 2; rows with differing environments → `IDENTITY` 2; unwritable `latestPath` → `WRITE_FAILED` 2 with the outcome still printed — assert that printed outcome, returned exit code, summary text, and `latest.json.outcome` agree, and that a pre-existing stale `latest.json` and rows dir are removed before the child runs. `summary.test.ts`: the markdown carries outcome, identity, per-row table, and the fixed advisory sentence; `writeStepSummary` appends only when the env var is set. Run — expect: module not found.
- [ ] **Step 2:** Implement `summary.ts` and `run.ts` (`--list` passthrough; clean start; spawn with `stdio: ["inherit","pipe","pipe"]`, relay + retain; decide; `finalizeOutcome`). Run — expect: pass.
- [ ] **Step 3:** Add the `perf:proxy` script and append the manifest entry; verify with `bun run verify:frontend --list` (seven entries) and `bun run verify:frontend --profile perf-proxy` (row shows `fail 2` with `NO_BASELINE`, matrix exits 0); `bun run verify:frontend` with no arguments — expect: no WebKit launch (no `perf-proxy` row). `git diff scripts/verify-frontend.manifest.json` — expect: only an appended object.
- [ ] **Step 4:** Commit: `feat(perf-proxy): add outcome wrapper and register the perf-proxy profile`.

_Requirements: PERFPROXY-1.12, PERFPROXY-2.4, PERFPROXY-2.5, PERFPROXY-2.6, PERFPROXY-2.7, PERFPROXY-2.8, PERFPROXY-4.3, PERFPROXY-5.1, PERFPROXY-5.2, PERFPROXY-5.3_

---

### Task 5: Explicit recalibration and the first local baseline

**Files:**
- Create: `desktop-ui/scripts/perf-proxy/recalibrate.ts`, `desktop-ui/tests/perf-proxy/baselines/<environmentKey>.json` (this machine)
- Modify: `desktop-ui/package.json` (script `perf:proxy:recalibrate` = `bun scripts/perf-proxy/recalibrate.ts`)
- Test: `desktop-ui/scripts/perf-proxy/recalibrate.test.ts`

**Reuse:** rung 2 — `run.ts`' measure step exported as `measureOnce()`; `contract.ts` constants; rung 7 — the median/spread/margin aggregation is new code (design § Recalibration).

**Interfaces:**
- Consumes: `measureOnce(): Promise<{ rows: RowResult[]; identity }>` (exported from Task 4's `run.ts`), `MARGIN_POLICY`, `R_REPETITIONS` (Task 1).
- Produces: `aggregate(reps: RowResult[][]): Baseline["rows"]`; CLI `bun run perf:proxy:recalibrate [--candidate <dir>]`.

**Depends-on:** Task 4

- [ ] **Step 1:** Failing tests with an injected `measureOnce`: R results → per row `median`/`spread`/`margin` per the policy (margin = max(multiplier × spread, floor)); repetitions with differing identities → `COULD_NOT_MEASURE / IDENTITY` exit 2; without `--candidate` the file lands at `tests/perf-proxy/baselines/<key>.json`; with `--candidate <dir>` nothing is written under `tests/`; and a static check that `run.ts` recognizes only `--list`: `grep -o -- '"--[a-z-]*"' scripts/perf-proxy/run.ts | sort -u` equals `"--list"` (no `--recalibrate`, `--candidate`, or `--write-baseline` token). Run — expect: module not found.
- [ ] **Step 2:** Implement; run — expect: pass. Then `cd desktop-ui && bun run perf:proxy:recalibrate` on this machine, commit the produced baseline, and confirm `bun run perf:proxy` now reports `HEALTHY` exit 0.
- [ ] **Step 3:** Commit: `feat(perf-proxy): add explicit recalibration and the first local baseline`.

_Requirements: PERFPROXY-3.1, PERFPROXY-3.2, PERFPROXY-3.3, PERFPROXY-3.4_

---

### Task 6: Rendering-proxy workflow

**Files:**
- Create: `.github/workflows/rendering-proxy.yml`

**Reuse:** rung 2 — `ci.yml:50-72` step shape (checkout, setup-bun, `bun install --frozen-lockfile`); rung 4 — `on.<event>.paths`, `workflow_dispatch.inputs`, `actions/upload-artifact@v4` (`release-build-artifacts.yml:185`), `$GITHUB_STEP_SUMMARY` (design § Rendering-proxy workflow).

**Interfaces:**
- Consumes: `bun run perf:proxy`, `bun run perf:proxy:recalibrate --candidate perf-proxy-candidate`, `bun run perf:proxy:typecheck` (Tasks 2, 4, 5).
- Produces: workflow `rendering-proxy` with `permissions: { contents: read }`, triggers push/pull_request on `main`, `dev` with `paths: [desktop-ui/**, packages/design-system/**, scripts/verify-frontend.manifest.json, .github/workflows/rendering-proxy.yml]`, `workflow_dispatch` input `recalibrate` (boolean, default false); steps: checkout → setup-bun → install → `bunx playwright install webkit` → typecheck → routine run (`if: !inputs.recalibrate`) or recalibration (`if: inputs.recalibrate`) → `if: always()` upload `desktop-ui/test-results/perf-proxy/latest.json` as `perf-proxy-latest` and `perf-proxy-candidate/` as `perf-proxy-baseline-candidate` when present.

**Depends-on:** Task 4, Task 5

- [ ] **Step 1:** Write the workflow; validate with `python3 -c 'import yaml;w=yaml.safe_load(open(".github/workflows/rendering-proxy.yml"));print(list(w["on"]), list(w["jobs"]))'` — expect: `['push','pull_request','workflow_dispatch'] ['rendering-proxy']`.
- [ ] **Step 2:** Push the branch and open the PR; confirm the job ran (paths matched), the step summary shows `COULD_NOT_MEASURE / NO_BASELINE` with identity and the advisory sentence, and `perf-proxy-latest` was uploaded; note the job's wall-clock duration in the task report.
- [ ] **Step 3:** Commit: `ci(perf-proxy): add the path-filtered rendering-proxy workflow`.

_Requirements: PERFPROXY-4.1, PERFPROXY-4.2, PERFPROXY-4.4, PERFPROXY-4.5, PERFPROXY-4.7, PERFPROXY-7.2_

---

### Task 7: Disposition protocol and docs

**Files:**
- Modify: `docs/standards/frontend-performance.md` (new section after line 61), `docs/standards/desktop-ui.md` (`## Verify matrix`, lines 32–51), `docs/agents/project.md` (line 109)

**Reuse:** rung 2 — extend the three existing documents at the anchors the scan found (design § Docs).

**Interfaces:**
- Consumes: outcome codes, scripts, manifest entry, artifact names (Tasks 4–6).
- Produces: `## Rendering proxy lane (advisory)` in `frontend-performance.md` (outcomes; required human record per outcome; "disposition required, HEALTHY not a shipping condition"; ROAD-5 authority; `latest.json` versioned evidence format, referenced not parsed); `desktop-ui.md` gains the manifest entry, profile, three scripts, exit codes, the two-step bootstrap; `project.md:109` reads "Browser E2E (Playwright, Chromium smoke): `cd desktop-ui && bun run test:e2e`; rendering proxy (Playwright WebKit, report-only): `cd desktop-ui && bun run perf:proxy`".

Neighbor note: `project.md` and `desktop-ui.md` are FVM-owned and hash-pinned by FVM's plan; edits stay inside the named anchors.

**Depends-on:** Task 4

- [ ] **Step 1:** Write the three edits; `git diff --stat` — expect: only the three files; `grep -c "advisory WebKit proxy" docs/standards/frontend-performance.md` — expect: ≥1; the FVM verify-table rows in `project.md` unchanged (`git diff docs/agents/project.md` touches line 109 only).
- [ ] **Step 2:** Commit: `docs(perf-proxy): write the advisory disposition protocol and lane docs`.

_Requirements: PERFPROXY-6.1, PERFPROXY-6.2, PERFPROXY-6.3, PERFPROXY-6.4_

---

### Task 8: Final spike, acceptance, and bootstrap

**Files:**
- Modify: `desktop-ui/scripts/perf-proxy/contract.ts` (only if the spike changes `MARGIN_POLICY`, `R_REPETITIONS`, or `K_CAPTURES`), `desktop-ui/tests/perf-proxy/baselines/` (re-recorded local baseline; the reviewed CI candidate committed)

**Reuse:** rung 2 — FVM's `acceptance.sh` pattern (`desktop-ui/scripts/verify-matrix/acceptance.sh`, hand-rolled bash, `set -euo pipefail`) for the one-time procedure (design § Acceptance procedure).

**Interfaces:**
- Consumes: everything above.
- Produces: the acceptance record in the task report; the committed CI-environment baseline.

**Depends-on:** Task 5, Task 6, Task 7

- [ ] **Step 1:** Final repeatability spike: run `bun run perf:proxy:recalibrate` (R = 5) then five consecutive `bun run perf:proxy` on the unchanged tree — expect: five `HEALTHY`; record per-row spreads; if any row's margin from the policy is below the observed spread, adjust `MARGIN_POLICY` in `contract.ts`, re-run tests, recalibrate, and repeat the five runs.
- [ ] **Step 2:** Distinguishing power: in an uncommitted copy of `rows.perf.ts`, inject the spike's Lens-like load (`addStyleTag` with the animated blurred ground + three `backdrop-filter` panels) after the precondition; run `bun run perf:proxy` — expect: `DEGRADED` on `idle-*` rows; discard the copy.
- [ ] **Step 3:** Boundary diffs against `c3c50f052202f062fa889973c44b5dabe3f8e704`: `git diff --exit-code <base> -- desktop-ui/playwright.config.ts desktop-ui/tests/smoke desktop-ui/scripts/verify-matrix`; `test:e2e` script line unchanged; first six manifest entries equal: `python3 -c "import json,subprocess;b=json.loads(subprocess.run(['git','show','<base>:scripts/verify-frontend.manifest.json'],capture_output=True,text=True).stdout)[:6];c=json.load(open('scripts/verify-frontend.manifest.json'))[:6];print('equal:',b==c)"` — expect: `equal: True` and all diffs clean.
- [ ] **Step 4:** Branch protection: `gh api repos/KlyntLabs/klyntbot/branches/main/protection` and the same for `dev` — expect: either a 404 `Branch not protected` (no required checks exist; record it as a pass) or a protection object whose required status checks do not include `rendering-proxy`; record the output.
- [ ] **Step 5:** Bootstrap: dispatch the workflow with `recalibrate: true`, download `perf-proxy-baseline-candidate`, review, commit it under `tests/perf-proxy/baselines/`; push and confirm the next routine CI run reports `HEALTHY`.
- [ ] **Step 6:** Commit: `test(perf-proxy): record acceptance, margins, and the CI baseline`.

_Requirements: PERFPROXY-4.6, PERFPROXY-5.7, PERFPROXY-5.8, PERFPROXY-7.3, PERFPROXY-7.4_
