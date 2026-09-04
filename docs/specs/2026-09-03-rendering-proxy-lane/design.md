# Design: Rendering proxy lane

Feature code: PERFPROXY
Status: In-progress
Date: 2026-09-04
Requirements: ./requirements.md

## Context

The verify matrix (FVM, Implemented) runs six frontend checks from a JSON manifest and already dispatches by profile: `selectChecks` defaults to `default`, `--profile <name>` selects any other, and a `report` row's non-zero exit is shown as `fail <code>` without changing the matrix exit code (`desktop-ui/scripts/verify-matrix/matrix.ts:156, 279-291, 300-324`). The manifest has no `report` row and no non-`default` profile yet; this lane adds the first of each. FVM relays a child's output to the console and never parses it, so anything the lane wants a reader to see in CI has to be written by the lane itself.

Playwright today is one config, one chromium project, one smoke spec, with `bun run dev` as its web server. Its types allow everything the requirements lock: a second config chosen with `--config`, `testMatch: "**/*.perf.ts"`, `outputDir`, `webServer.reuseExistingServer: false` (which throws when the port is busy), `page.route` with `route.fulfill({ json })`, `page.addInitScript`, and `page.screenshot({ animations: "allow", scale, type })` returning a `Buffer`. The chat page's two rendering paths have roots that differ only in class names and both carry `aria-live="polite"`; react-window's outer element adds inline styles and no attribute. The app reads its theme from `localStorage["klynt-theme"]`, not from `prefers-color-scheme`.

The binding constraint is trust in the number. The run-spike showed WebKit's rAF cadence is repeatable to its 1 ms quantum but blind to compositor load, while screenshot capture latency sees a Lens-like load at 3.5× signal-to-noise; the requirements therefore record both, and every comparison is baseline-relative on an identical identity. That rules out the simplest shape, a Playwright spec that `expect`s "not degraded": Playwright exits 1 for any failing test, so a broken fixture and a real regression would be indistinguishable, and a reporter that calls `process.exit` truncates Playwright's own flushing. The design instead makes the specs responsible only for measuring and writing one row file each, and puts identity, comparison, outcome, and exit code in a small wrapper that runs under Bun after the Playwright child exits.

A second constraint is type safety in a feature whose binding constraint is trust in the number. No `tsconfig` covers `desktop-ui/tests/` or `desktop-ui/scripts/`, `@types/node` is not installed, and the `@shared/*` alias does not resolve outside Vite. The lane therefore adds `@types/node` as a development dependency (a new dependency, decided by the tech lead at design review), imports shared types relatively, and type-checks both `tests/perf-proxy/**/*.ts` and `scripts/perf-proxy/**/*.ts` with a dedicated `tsconfig.perf.json` (`types: ["node"]`, `lib: ["ES2023", "DOM"]`, `skipLibCheck: true`). The wrapper, identity, comparison, and recalibration code are typed, not a ledger item. No `docs/architecture/` invariant applies. Fresh retrieval found FVM as the only neighbor (term "rendering proxy") and no owner for any lane path. No ADR: the metric amendment is documented in the requirements context and the spike record, and it is cheap to reverse.

## Decisions

1. **Specs measure, the wrapper judges.** Each of the six rows is one Playwright test in `desktop-ui/tests/perf-proxy/rows.perf.ts` that writes `test-results/perf-proxy/rows/<row>.json`. The wrapper `desktop-ui/scripts/perf-proxy/run.ts` spawns `playwright test --config playwright.perf-proxy.config.ts`, merges the row files into `latest.json`, resolves identity, compares against the baseline, prints the outcome, writes the step summary, and exits 0/1/2.
2. **Row files, not a reporter.** Per-row files survive a row that crashes (the missing file becomes `COULD_NOT_MEASURE / ROW_MISSING`), and no reporter has to reason about `process.exit`.
3. **A neutral `data-render-path` attribute** (`"plain"` | `"virtualized"`) is added to each list root, one line each. It is always present, names no test concept, and is the precondition signal PERFPROXY-1.6 needs; content checks prove the fixture loaded.
4. **Fixed screenshot options**: `{ type: "png", scale: "css", animations: "allow", caret: "hide", fullPage: false }` on the 1280×800 viewport at DPR 1, in memory, K captures. Recorded verbatim in every row.
5. **Identity.** `environmentKey` = first 16 hex of SHA-256 over the canonical JSON of `{ runnerClass, platform, arch, osMajor, webkitVersion, headless, viewport, dpr }` where `runnerClass` is `github:<ImageOS>` on GitHub Actions and `local:<full sha256 hex of hostname>` elsewhere; only the final key is truncated. This prevents accidental baseline sharing between machines with different hostnames and guarantees nothing across identical hostnames (PERFPROXY-3.5 as amended). The raw hostname is never serialized: `describeEnvironment` returns the digest only, and a unit test scans a produced baseline, candidate, `latest.json`, filename, and summary for the hostname string. `schemaVersion` and `measurementContractVersion` are constants in `contract.ts`.
6. **Margins live in the baseline, policy in code.** Recalibration stores, per row and metric, the median and spread across R repetitions and the margin the policy derived from them. `compare.ts` applies the stored margin only. Candidate policy constants from the spike (rAF p95: max(3 × spread, 2 ms); screenshot p50: max(3 × spread, 15 %)) are placeholders the final spike task confirms or replaces before the first recalibration lands.
7. **`--list` is handled by the wrapper** and passed through to `playwright test --config … --list`; the FVM CLI cannot forward arguments to a child, so listing rows through the matrix is documented as unsupported.
8. **Type-check by a dedicated tsconfig** covering specs, fixtures, and wrapper scripts, run as `bun run perf:proxy:typecheck` in the workflow and in the acceptance procedure; `@types/node` is added as a devDependency for it.
9. **CI job is red on `DEGRADED` and `COULD_NOT_MEASURE`.** The lane is not a required check, so a red job blocks nothing, and a green job on a degradation would hide the signal. The artifact and summary upload with `if: always()`.
10. **R = 5 repetitions and K = 10 captures** are the defaults the final spike validates; they are constants in `contract.ts`, not flags.
11. **One finalization path.** Every terminal path of the wrapper, including early Playwright failure and `NO_BASELINE`, goes through `finalizeOutcome`, which fixes the outcome and subcode, writes `latest.json` atomically (temp file + rename) exactly once, prints the outcome block, writes the step summary, and returns the exit code. The printed outcome, exit code, summary, and `latest.json` therefore always agree whenever the file is writable.
12. **Clean start.** The wrapper deletes the rows directory and any previous `latest.json` before spawning Playwright, so a previous run's artifact can never be uploaded or compared.
13. **Rows carry their environment.** Each `RowResult` includes an `environment` block captured in the browser context; the wrapper requires all six rows (and, in recalibration, all repetitions) to agree, else `COULD_NOT_MEASURE / IDENTITY`.

## Architecture

Vocabulary for every section below: **module**, **interface**, **implementation**,
**seam** (public surface where behavior is observable and substitutable). Prefer
deep modules — an interface much simpler than what it hides.

```
bun run perf:proxy [--list]            bun run perf:proxy:recalibrate [--candidate <dir>]
        │                                       │
        ▼                                       ▼
scripts/perf-proxy/run.ts               scripts/perf-proxy/recalibrate.ts
  spawn playwright test --config …         run.ts' measure step × R
  merge rows → latest.json                 aggregate → baseline (tracked file | candidate dir)
  identity · compare · outcome · summary
        │ reads                                  │ writes
        ▼                                        ▼
test-results/perf-proxy/rows/*.json     tests/perf-proxy/baselines/<environmentKey>.json
        ▲
        │ written by
tests/perf-proxy/rows.perf.ts  (six tests: theme → mocks → navigate → precondition → sample → capture → row file)
        │ uses
tests/perf-proxy/support/{fixtures,sampler,preconditions,rowfile}.ts
```

### Perf Playwright config

Satisfies: PERFPROXY-1.1, PERFPROXY-2.8, PERFPROXY-5.4, PERFPROXY-5.5, PERFPROXY-5.6
Reuse: rung 2 — `desktop-ui/playwright.config.ts` as the shape to mirror (webServer block, baseURL); rung 4 — Playwright config fields `testDir`, `testMatch`, `outputDir`, `projects`, `webServer` (`playwright/types/test.d.ts:803, 838, 10753`)
Interface: `desktop-ui/playwright.perf-proxy.config.ts` exporting `defineConfig({ testDir: "./tests/perf-proxy", testMatch: "**/*.perf.ts", outputDir: "test-results/perf-proxy/artifacts", fullyParallel: false, workers: 1, retries: 0, reporter: "list", use: { baseURL: "http://localhost:1420", trace: "off" }, projects: [{ name: "webkit", use: { ...devices["Desktop Safari"], viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 } }], webServer: { command: "bun run dev", url: "http://localhost:1420", reuseExistingServer: false, timeout: 60_000 } })`.
Depth: n/a — extends the existing config pattern
Locality: new file beside `playwright.config.ts`; `playwright.config.ts` — leave (PERFPROXY-5.7); `desktop-ui/package.json` — extend with `perf:proxy`, `perf:proxy:recalibrate`, `perf:proxy:typecheck` scripts (see CLI module).

Serial mode is a success-path guarantee: with `workers: 1` and `fullyParallel: false`, a completed measurement has run the six rows in order in one browser; after a row failure Playwright may skip the rest, and the wrapper reports `COULD_NOT_MEASURE` (PERFPROXY-1.1 as amended). Isolation is structural: the default config's `testDir ./tests` with the default `testMatch` never matches `*.perf.ts`, and this config's `testMatch` never matches `*.spec.ts`. PERFPROXY-5.5/5.6 are verified by running both `--list` commands; `playwright test --list` does not start the `webServer` (verified with port 1420 free), so listing needs no server. The viewport and device scale factor are set inside the project's `use` after spreading `devices["Desktop Safari"]`, because a project's `use` overrides the top-level `use` per key and the device preset carries 1280×720 at DPR 2.

### Fixtures and route allowlist — `tests/perf-proxy/support/fixtures.ts`

Satisfies: PERFPROXY-1.3, PERFPROXY-1.4, PERFPROXY-1.11
Reuse: rung 2 — `ChatMessage` and `ChatThread` from `desktop-ui/src/shared/types/chat.ts` (relative import), `AppInfoResponse` from `src/shared/types/common.ts`; rung 4 — `page.route` + `route.fulfill({ json })` (`types.d.ts:22601`), `page.addInitScript` (`types.d.ts:327`)
Interface: `buildMessages(n: number): ChatMessage[]` (ids `m1…mn`, role alternating user/assistant, fixed timestamps from a constant epoch, content `"msg <i> — " + filler` with no markdown); `buildThread(sessionKey, n): ChatThread`; `ALLOWLIST: Record<string, (req: { sessionKey?: string }) => unknown>` for the nine commands (`app_info` → `{ setupCompleted: true, … }`, `chat_threads` → `[thread]`, `chat_messages` → `buildMessages(n)`, `view_clear_active` and `autotuner_status` → `null`, `flashcard_total_due`, `journey_item_count`, `autotuner_get_toast_count` → `0`, `journey_milestones` → `[]`); `installMocks(page, { sessionKey, n }): { unexpected: string[] }` which registers `page.route("**/api/**")`, answers allowlisted commands, fulfills anything else with status 500 and records its name, and stubs `window.EventSource` via `addInitScript` (SSE is not mocked by routing).
Depth: if this module vanished, callers would need to know the nine command names, the three response shapes, and that SSE must be stubbed — the allowlist table and one init script, nothing about Playwright routing internals.
Locality: new files under `desktop-ui/tests/perf-proxy/support/`; `desktop-ui/src` — leave (no fixture mode); `src/shared/types/chat.ts` — leave (imported).

`unexpected` is asserted empty in the precondition step so the row fails naming the command (PERFPROXY-1.4); the 500 also makes the app's own failure visible in the relayed output.

### Preconditions and theme — `tests/perf-proxy/support/preconditions.ts`

Satisfies: PERFPROXY-1.2, PERFPROXY-1.5, PERFPROXY-1.6
Reuse: rung 2 — `ThemeProvider.tsx:20/34` (reads `localStorage["klynt-theme"]` synchronously into state; sets `html[data-theme]` in an effect, so the attribute lands one tick after first render and `expectTheme` waits for it); rung 4 — `page.addInitScript`, `page.waitForFunction`, `locator.waitFor`
Surface: `MessageList.tsx:135` root and `VirtualizedMessageList.tsx:336` root gain `data-render-path="plain"` / `"virtualized"` — readers: none today (no test or style references these roots by attribute) → `replace` (new attribute, nothing migrated).
Interface: `setTheme(context, theme)` (init script writing `klynt-theme`); `expectTheme(page, theme)` (waits for `html[data-theme]` to equal the theme); `expectPlainRendered(page, messages)` (waits for `[data-render-path="plain"]` and for each message's content to be present); `expectVirtualizedRendered(page, messages)` (waits for `[data-render-path="virtualized"]` and for the last message's content to be visible).
Depth: n/a — thin waits over existing DOM facts; the one new fact is the attribute name, held in one constant.
Locality: new support file; `MessageList.tsx` and `VirtualizedMessageList.tsx` — extend by one attribute each on the existing root element; `ChatPage.tsx` — leave.

### In-page sampler — `tests/perf-proxy/support/sampler.ts`

Satisfies: PERFPROXY-1.7, PERFPROXY-1.8, PERFPROXY-1.9, PERFPROXY-1.10
Reuse: rung 2 — the spike's sampler shape (`.skills/PERFPROXY/spike-repeatability.md`) reimplemented under test-first; rung 4 — `requestAnimationFrame`, `performance.now`, `page.evaluate`, `page.screenshot` (`types.d.ts:4464`); rung 7 — the percentile statistics and scroll driver are new code
Interface: `sampleRaf(page, { warmupFrames: 60, sampleFrames: 300, scroll?: { stepPx: 120 } }): Promise<number[]>` — a function passed to `page.evaluate` that discards `warmupFrames` gaps, collects `sampleFrames` gaps, and when `scroll` is set moves the virtualized scroller (`[data-render-path="virtualized"] > div`, react-window's outer element, `react-window/src/createListComponent.js:363-379`) from its bottom toward the top by `stepPx` per frame; `captureScreenshots(page, k): Promise<number[]>` timing K `page.screenshot(SCREENSHOT_OPTIONS)` calls; `stats(values): { p50, p95, max }` (nearest-rank percentiles); `SCREENSHOT_OPTIONS` constant (Decision 4).
Depth: if this module vanished, callers would need to know "discard 60, sample 300 rAF gaps, optional 120 px per-frame scroll of the virtualized scroller, then K timed screenshots with the fixed options" — the contract sentence, not the DOM walking or percentile code.
Locality: new support file; nothing else changes.

The 600 ms waits before sampling and before captures are performed by the spec (PERFPROXY-1.7, 1.9), not the sampler, so the sequence is visible in one place.

### Row specs — `tests/perf-proxy/rows.perf.ts`

Satisfies: PERFPROXY-7.1
Reuse: rung 2 — `tests/smoke/theme-toggle.spec.ts` as the spec shape; rung 4 — `test.describe.configure({ mode: "serial" })` (`test.d.ts:4226`), `test.info()`
Interface (writes the six row files the wrapper aggregates for PERFPROXY-1.12): six generated tests named `<scenario>×<theme>` from a `ROWS` table `[{ scenario: "idle-20", n: 20, scroll: false }, { scenario: "idle-200", n: 200 }, { scenario: "scroll-200", n: 200, scroll: true }] × ["light", "dark"]`. Each test: new context (theme init script, `reducedMotion: "no-preference"`), `installMocks`, `goto("/#/chat?thread=<sessionKey>")`, `expectTheme`, precondition, 600 ms, `sampleRaf`, 600 ms, `captureScreenshots`, then `writeRowFile(row)`. Row shape: `{ row, scenario, theme, n, renderPath, raf: { sampleCount, p50Ms, p95Ms, maxMs }, screenshot: { captureCount, p50Ms, options }, environment: { webkitVersion (from browser.version()), headless, viewport, dpr, userAgent }, durationMs }`. The `environment` block is what `env.ts` consumes; rows that disagree are rejected (Decision 13).
Depth: n/a — composition of the support modules; the only knowledge a caller needs is the `ROWS` table and the row-file location, both exported from `support/rowfile.ts`.
Locality: new spec file; `desktop-ui/tests/smoke/` — leave.

### Row file I/O — `tests/perf-proxy/support/rowfile.ts`

Satisfies: PERFPROXY-1.12
Reuse: rung 3 — `node:fs` `mkdirSync`/`writeFileSync`/`readdirSync`
Interface: `ROWS_DIR = "test-results/perf-proxy/rows"`; `writeRowFile(row: RowResult): void` (JSON, one file per row name); `readRowFiles(dir): RowResult[]`; `EXPECTED_ROWS: string[]` (the six names). Shared by specs (write) and wrapper (read).
Depth: if this module vanished, callers would need the directory and "one JSON file per row, named by the row" — two facts.
Locality: new support file; `.gitignore` — leave (`desktop-ui/test-results/` already ignored, `.gitignore:55`).

### Contract, identity, and comparison — `scripts/perf-proxy/{contract,env,compare}.ts`

Satisfies: PERFPROXY-2.1, PERFPROXY-2.2, PERFPROXY-2.3, PERFPROXY-2.4, PERFPROXY-3.5
Reuse: rung 3 — `node:os` (`platform`, `arch`, `release`, `hostname`, `cpus`), `node:crypto` `createHash`; rung 4 — `browser.version()` passed in from the spec via the row file's `environment` block; rung 7 — the identity derivation, margin policy, and comparison logic are new code
Interface:
- `contract.ts`: `SCHEMA_VERSION = 1`, `MEASUREMENT_CONTRACT_VERSION = 1`, `R_REPETITIONS = 5`, `K_CAPTURES = 10`, `MARGIN_POLICY` (Decision 6), `Identity = { schemaVersion, measurementContractVersion, environmentKey }`, `Baseline`, `LatestRun`, `Outcome = "HEALTHY" | "DEGRADED" | "COULD_NOT_MEASURE"`, `Subcode` union (`NO_BASELINE`, `PRECONDITION`, `UNEXPECTED_COMMAND`, `LAUNCH`, `TIMEOUT`, `IDENTITY`, `ROW_MISSING`, `LATEST_MALFORMED`, `CHILD_FAILED`, `PORT_BUSY`, `WRITE_FAILED`).
- `env.ts`: `describeEnvironment(input: { env: NodeJS.ProcessEnv; os: OsFacts; rows: RowResult[] }): { key: string; keyed: KeyedDims; diagnostic: Diagnostics }` — pure given its inputs; takes the browser facts from the rows' `environment` blocks and throws `IdentityMismatch` when they disagree; `runnerClass` per Decision 5; the hostname enters only as its full digest.
- `compare.ts`: `compareRows(latest: LatestRun, baseline: Baseline): { outcome: "HEALTHY" | "DEGRADED"; rows: RowDelta[] }` — pure; `identityMatches(a, b): boolean`.
Depth: if these vanished, callers would need the identity triple, the two compared metrics, and "within the baseline's stored margin" — three sentences, not the hashing, percentile, or policy code.
Locality: new files under `desktop-ui/scripts/perf-proxy/`; `desktop-ui/scripts/verify-matrix/` — leave (PERFPROXY-5.8).

### Wrapper CLI — `scripts/perf-proxy/run.ts`

Satisfies: PERFPROXY-2.5, PERFPROXY-2.6, PERFPROXY-2.7, PERFPROXY-5.1, PERFPROXY-5.2, PERFPROXY-5.3
Reuse: rung 2 — FVM's `main.ts` argv shape and its `spawn` injection pattern (`desktop-ui/scripts/verify-matrix/main.ts:19-48`, `matrix.ts:187-248`), FVM's `main.test.ts` spawn-in-test pattern; rung 3 — `node:child_process` `spawn`, `node:fs`; rung 7 — the outcome state machine and subcodes are new code
Surface: `scripts/verify-frontend.manifest.json` gains one entry (`perf-proxy`) — readers: FVM's `loadManifest` (validates shape, `matrix.ts:52, 86-97`) → `replace` (additive; the six existing entries byte-identical, PERFPROXY-5.8); `desktop-ui/package.json` `scripts` gains three keys — readers: humans, `desktop-ui.md` scripts table → `replace` (additive; `test:e2e` untouched, PERFPROXY-5.7).
Interface: `bun run perf:proxy` → `run.ts`: (0) if `--list`, exec `playwright test --config playwright.perf-proxy.config.ts --list` and exit with its code; (1) delete `ROWS_DIR` and any existing `latest.json` (Decision 12); (2) spawn `playwright test --config …` with `stdio: ["inherit", "pipe", "pipe"]`, relaying stdout and stderr live and retaining them (FVM's `runOne` pattern); (3) collect whatever row files exist and compute the identity when the rows allow it; (4) decide: non-zero child → `COULD_NOT_MEASURE` (`PORT_BUSY` when the retained output contains Playwright's "is already used" line, `runner/index.js:851`; else `CHILD_FAILED`); a missing expected row → `ROW_MISSING`; disagreeing row environments → `IDENTITY`; no baseline for the identity → `NO_BASELINE`; otherwise `compareRows` → `HEALTHY` or `DEGRADED`; (5) `finalizeOutcome({ outcome, subcode?, rows, identity?, comparison? })` — builds the `LatestRun` envelope with the final outcome, writes it atomically (temp file then rename) exactly once, prints the outcome block, writes the step summary when `GITHUB_STEP_SUMMARY` is set, and returns 0 / 1 / 2; if the write itself fails, the outcome becomes `COULD_NOT_MEASURE / WRITE_FAILED` and the summary is still attempted. `runProxy(opts: { spawn?, rowsDir?, baselinesDir?, latestPath?, env? }): Promise<{ outcome, subcode?, exitCode }>` is the testable entry; `main()` maps it to `process.exit`.
Depth: if `run.ts` vanished, callers would need the step order above and the three exit codes — the outcome contract, which is what the requirements already state.
Locality: new file; `desktop-ui/scripts/verify-matrix/` — leave; root `package.json` — leave (the matrix already reaches `desktop-ui` scripts through the manifest).

### Recalibration — `scripts/perf-proxy/recalibrate.ts`

Satisfies: PERFPROXY-3.1, PERFPROXY-3.2, PERFPROXY-3.3, PERFPROXY-3.4
Reuse: rung 2 — `run.ts`' measure step (steps 2–6) exported as `measureOnce()`; `contract.ts` constants; rung 7 — the median/spread/margin aggregation is new code
Interface: `bun run perf:proxy:recalibrate [--candidate <dir>]` → runs `measureOnce()` R times, requires every repetition's identity to be equal (else `COULD_NOT_MEASURE / IDENTITY`, exit 2), aggregates per row and metric (`median`, `spread = max − min`, `margin` from `MARGIN_POLICY`), builds `Baseline = { identity, recordedAt, sourceRevision (git rev-parse HEAD), repetitions: R, rows: { [row]: { raf: { p95: { median, spread, margin } }, screenshot: { p50: { median, spread, margin } } } }, environment: { keyed, diagnostic } }`; without `--candidate` writes `tests/perf-proxy/baselines/<environmentKey>.json`; with `--candidate <dir>` writes `<dir>/<environmentKey>.json` and nothing under `tests/`. `run.ts` has no write path to `baselines/` (PERFPROXY-3.4).
Depth: if this vanished, callers would need "R measured runs, per-metric median/spread/margin, one JSON per identity, tracked or candidate" — the baseline shape, which `contract.ts` types.
Locality: new file; `tests/perf-proxy/baselines/` — new directory, tracked, initially holding one local baseline produced during the feature's acceptance.

### Step summary — `scripts/perf-proxy/summary.ts`

Satisfies: PERFPROXY-4.3
Reuse: rung 3 — `node:fs` `appendFileSync`
Interface: `renderSummary(run: LatestRun, comparison?: RowDelta[], subcode?): string` (markdown: outcome heading, identity line, per-row table of baseline / current / delta for both metrics, the fixed sentence "This result is an advisory WebKit proxy, not native rendering evidence."); `writeStepSummary(markdown, env)` appends to `$GITHUB_STEP_SUMMARY` when set, else no-op.
Depth: if this vanished, callers would need the four blocks the summary carries — stated in PERFPROXY-4.3.
Locality: new file; nothing else changes.

### Rendering-proxy workflow — `.github/workflows/rendering-proxy.yml`

Satisfies: PERFPROXY-4.1, PERFPROXY-4.2, PERFPROXY-4.4, PERFPROXY-4.5, PERFPROXY-4.7, PERFPROXY-7.2
Reuse: rung 2 — `ci.yml:50-72` step shape (checkout, setup-bun, `bun install --frozen-lockfile`); rung 4 — `on.<event>.paths`, `workflow_dispatch.inputs`, `actions/upload-artifact@v4` (`release-build-artifacts.yml:185`), `$GITHUB_STEP_SUMMARY`
Interface: `on: { push: { branches: [main, dev], paths: [desktop-ui/**, packages/design-system/**, scripts/verify-frontend.manifest.json, .github/workflows/rendering-proxy.yml] }, pull_request: { same }, workflow_dispatch: { inputs: { recalibrate: { type: boolean, default: false } } } }`; `permissions: { contents: read, actions: write }` (`actions: write` is required for `actions/upload-artifact`); one job `rendering-proxy` on `macos-latest`: checkout → setup-bun → install → `bunx playwright install webkit` → `bun run perf:proxy:typecheck` → (`if: !inputs.recalibrate`) `bun run perf:proxy` → (`if: inputs.recalibrate`) `bun run perf:proxy:recalibrate --candidate perf-proxy-candidate` → (`if: always()`) upload `desktop-ui/test-results/perf-proxy/latest.json` as `perf-proxy-latest` and, when present, `perf-proxy-candidate/` as `perf-proxy-baseline-candidate`. Because the wrapper deletes any previous `latest.json` before measuring (Decision 12), an uploaded file is always this run's envelope. Branch-protection state is external; PERFPROXY-4.6 is verified at acceptance (see the acceptance procedure), not by this workflow.
Depth: n/a — extends the CI shape
Locality: new workflow file; `ci.yml` — leave.

### Type-check config — `desktop-ui/tsconfig.perf.json`

Satisfies: none — infrastructure (the fixture type-check mechanism named by the requirements' Open Questions; the fixtures it checks are PERFPROXY-1.3's)
Reuse: rung 2 — `tsconfig.node.json` as the shape; rung 4 — `tsc --noEmit -p`; rung 5 — `@types/node` (new devDependency, decided at design review)
Interface: `{ compilerOptions: { strict: true, noEmit: true, skipLibCheck: true, module: "ESNext", moduleResolution: "bundler", target: "ES2022", lib: ["ES2023", "DOM"], types: ["node"] }, include: ["tests/perf-proxy/**/*.ts", "scripts/perf-proxy/**/*.ts"] }`; script `perf:proxy:typecheck` = `tsc --noEmit -p tsconfig.perf.json`.
Surface: `desktop-ui/package.json` `devDependencies` gains `@types/node`; `bun.lock` updated — readers: `bun install --frozen-lockfile` in every CI job → `replace` (the lockfile is committed with the dependency).
Depth: n/a — configuration
Locality: new file; `tsconfig.app.json` — leave; wrapper scripts, fixtures, and specs are all included.

### Docs — `frontend-performance.md`, `desktop-ui.md`, `project.md`

Satisfies: PERFPROXY-6.1, PERFPROXY-6.2, PERFPROXY-6.3, PERFPROXY-6.4
Reuse: rung 2 — extend the three existing documents at the anchors the scan found (`frontend-performance.md` after `## Measurement, not vibes`, line 61; `desktop-ui.md:32-51` `## Verify matrix`; `project.md:109`)
Surface: `desktop-ui.md` `## Verify matrix` — readers: FVM's docs cite it → `replace` (extended in place); `project.md:109` — readers: skills reading "Run locally" → `replace`.
Interface: a new `## Rendering proxy lane (advisory)` section in `frontend-performance.md` carrying the disposition protocol (outcomes, required record per outcome, "disposition required, HEALTHY not a shipping condition", ROAD-5 authority, `latest.json` as a versioned evidence format that is referenced not parsed); `desktop-ui.md` gains the manifest entry, profile, the three scripts, exit codes, and the two-step bootstrap; `project.md:109` reads "Browser E2E (Playwright, Chromium smoke): … ; rendering proxy (Playwright WebKit, report-only): `cd desktop-ui && bun run perf:proxy`".
Depth: n/a — documentation
Locality: extend the three files; `CLAUDE.md` — leave (it points at the standards).

### Acceptance procedure (infrastructure, no Satisfies of its own)

PERFPROXY-5.7, 5.8 (guards) are `git diff --exit-code <approved-base>` on the frozen paths plus a byte comparison of the first six manifest entries; PERFPROXY-4.6 is a human check of the branch-protection settings for `main` and `dev` (or `gh api repos/{owner}/{repo}/branches/{branch}/protection`) recorded in the review; PERFPROXY-7.3, 7.4 are one-time runs recorded in the review: five consecutive `bun run perf:proxy` after a local recalibration, then one run with the spike's Lens-like injection applied through a temporary `addStyleTag` in a copy of the spec (never committed). Mirrors FVM's `acceptance.sh` pattern; `plan-tasks` turns it into tasks.

## Seams for testing

| Seam | Kind | Covers |
|---|---|---|
| `describeEnvironment(input)` with injected env/os facts and row environments — including disagreeing rows → `IdentityMismatch`, and a scan of every produced file for the hostname | unit (vitest, node env) | PERFPROXY-3.5 |
| `compareRows(latest, baseline)`, `identityMatches` with fixture JSON | unit | PERFPROXY-2.1, PERFPROXY-2.2, PERFPROXY-2.3 |
| `runProxy({ spawn, rowsDir, baselinesDir, latestPath, env })` with a fake child and temp dirs — one consistency table asserting, per terminal path, that printed outcome, exit code, summary, and `latest.json` agree, and that a stale `latest.json` is removed first | unit | PERFPROXY-2.4, PERFPROXY-2.5, PERFPROXY-2.6, PERFPROXY-2.7, PERFPROXY-2.8 (fake child emitting "is already used"), PERFPROXY-1.12 (merge), PERFPROXY-3.4 |
| `renderSummary`, `writeStepSummary` with a temp `GITHUB_STEP_SUMMARY` file | unit | PERFPROXY-4.3 |
| `recalibrate` aggregation with fake `measureOnce` results | unit | PERFPROXY-3.1, PERFPROXY-3.2, PERFPROXY-3.3 |
| `buildMessages` / `buildThread` / `ALLOWLIST` shapes | unit (vitest) + `tsc -p tsconfig.perf.json` | PERFPROXY-1.3, PERFPROXY-1.11 |
| `rows.perf.ts` against Vite in WebKit (the lane itself) | e2e | PERFPROXY-1.1, 1.2, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 1.10, 1.12 (row files), 2.8 (confirmation only), 7.1 |
| `--list` on both configs | integration (CLI) | PERFPROXY-5.4, PERFPROXY-5.5, PERFPROXY-5.6 |
| matrix run `bun run verify:frontend` / `--profile perf-proxy` | integration (CLI) | PERFPROXY-5.1, PERFPROXY-5.2, PERFPROXY-5.3 |
| workflow run on the feature PR + one manual dispatch | integration (CI) | PERFPROXY-4.1, 4.2, 4.4, 4.5, 4.7, 7.2 |
| branch-protection settings read by a human or `gh api` | acceptance | PERFPROXY-4.6 |
| acceptance runs (five HEALTHY, one injected DEGRADED) | acceptance, once | PERFPROXY-7.3, PERFPROXY-7.4 |
| review-time diffs | acceptance | PERFPROXY-5.7, PERFPROXY-5.8 |
| doc review | manual | PERFPROXY-6.1, 6.2, 6.3, 6.4 |

New seams: two — the `spawn` injection on `runProxy` (same pattern as FVM's) and the `measureOnce` injection on `recalibrate`. Everything else is a file, a CLI, CI, or the DOM.

## Coverage check

Mapped to modules: 1.1–1.12, 2.1–2.8, 3.1–3.5, 4.1–4.5, 4.7, 5.1–5.6, 6.1–6.4, 7.1, 7.2. Deliberately unmapped to a module (guards, an external-state check, and one-time acceptance runs): 4.6, 5.7, 5.8, 7.3, 7.4 — covered by the acceptance rows of the seams table. 48 IDs in requirements.md; 43 mapped to modules, 5 on acceptance seams.
