# Design: Frontend verify matrix

Feature code: FVM
Status: Approved
Date: 2026-09-03
Requirements: ./requirements.md

## Context

The frontend has six checks that already work on their own (`typecheck`, `lint`, `test`, `check:tokens`, `check:performance`, `test:e2e` in `desktop-ui/package.json:6-21`) and a root `package.json` whose only job is to delegate into `desktop-ui/` (`test`, `typecheck`, `lint`, `check:tokens`, `check:performance`). Nothing runs all six together: CI's `desktop-ui-quality` job (`.github/workflows/ci.yml:50-74`) runs lint, typecheck, and unit tests; the verify table in `docs/agents/project.md:76-85` names only typecheck and lint for the frontend. The closest precedent for "run every gate, fail once at the end" is `scripts/run_chat_perf_gates.sh`, a bash script with a `FAIL` accumulator that runs each cargo bench independently and exits non-zero only after all have reported. The repo's precedent for testing a repo-root script is a hand-rolled bash test (`scripts/adapt_codex_vendor.sh.test.sh`) that nothing in CI runs.

The binding constraint is where tests can run. FVM-7.2 requires the runner's determinism to be proved by contract tests "in the frontend unit suite", and vitest is the only test runner in the repo. `desktop-ui/vitest.config.ts` picks up any `*.test.ts` under `desktop-ui/` except `tests/**`; nothing picks up a test file at repo-root `scripts/`, there is no root vitest config, no root devDependencies, and CI installs dependencies only inside `desktop-ui/`. That rules out a bash runner with bash tests (not in the unit suite) and rules out a TypeScript runner at repo root (no test harness, no install step). The runner therefore lives in `desktop-ui/scripts/verify-matrix/`, beside the existing `desktop-ui/scripts/check-performance-budget.sh`, and is executed by Bun, which the whole frontend already requires. The manifest stays at repo-root `scripts/` as FVM-2.1 requires, and the runner locates it from its own path.

A second constraint is the change boundary. FVM-5.1 freezes the `scripts` block of `desktop-ui/package.json` and FVM-6.x freezes the token gate, the Playwright config, the perf-budget script, and the Rust CI jobs. So the entry command is a new root `package.json` script, not a new `desktop-ui` script, and the runner touches no existing check: it spawns each check's existing package script in `desktop-ui/` and reports the exit status it gets back.

No `docs/architecture/` invariant applies: ARCH-1 concerns the desktop crate and this feature adds no Tauri or Rust code; ARCH-2 and ARCH-3 are untouched. Fresh catalog retrieval (neighbors of FVM) found no feature or observation owning a verify runner or CI gate; the only neighbor signal is EXPO's incidental mention of "vitest". No ADR is written: the one real trade-off, TypeScript under Bun instead of the repo's bash-script convention, is cheap to reverse and is fully explained by the test-harness constraint above.

## Decisions

1. **Runner is TypeScript executed by Bun, living in `desktop-ui/scripts/verify-matrix/`**, with contract tests colocated as `*.test.ts` so `bun run test` runs them. Reason: FVM-7.2 needs the tests in the vitest suite, and that is the only place vitest reaches.
2. **Manifest is JSON at `scripts/verify-frontend.manifest.json`**: an array of `{ name, command, cwd, mode, profiles }`. JSON parses natively in Bun with zero dependencies; FVM-2.1 fixes the directory.
3. **Entry command is `bun run verify:frontend`, a new root `package.json` script** that runs `bun desktop-ui/scripts/verify-matrix/main.ts`. It is the row in the verify table and the command CI calls. `desktop-ui/package.json` is not touched.
4. **Checks run sequentially in manifest order; each check's stdout and stderr are piped, relayed live to the matrix's own stdout/stderr, and retained on the check's row.** That is the literal reading of FVM-1.3: captured, and available both as it happens and afterwards. The runner never parses or counts the retained text. Piping means child reporters see a non-TTY and print their plain output, which is what CI logs show anyway. Sequential order also keeps `check:performance`'s `dist/` rebuild and `test:e2e`'s dev server from racing.
5. **The initial manifest declares all six checks `hard` with profiles `["default"]`.** No `quick` profile ships; the profile field exists so ROAD-4 and ROAD-5 register without deciding when they run (FVM-2.2).
6. **Exit status is passed through untouched.** The runner never reinterprets a check's exit code; that is what makes FVM-5.2 parity structural rather than empirical.
7. **CI installs the one Playwright browser the project declares (`chromium`) and ripgrep** before calling the matrix. The token gate shells out to `rg`, which is not guaranteed on `macos-latest`; a missing `rg` would surface as a spurious `check:tokens` failure.
8. **Parity and determinism over the real six checks are proved once, by an acceptance script**, not by every CI run. Contract tests with a temporary manifest and fake commands cover the runner's logic on every `bun run test`.
9. **The runner is not typechecked or linted in this feature.** `tsconfig.app.json` includes only `src/` and Biome includes only `src/**`; widening either changes what `typecheck` and `lint` do, which FVM-5.1 forbids here. Recorded as a ledger item for ROAD-6.

## Architecture

Vocabulary for every section below: **module**, **interface**, **implementation**,
**seam** (public surface where behavior is observable and substitutable). Prefer
deep modules — an interface much simpler than what it hides.

```
bun run verify:frontend [check …] [--profile <name>] [--list] [--manifest <path>]
        │  (root package.json)
        ▼
desktop-ui/scripts/verify-matrix/main.ts        CLI: parse argv → runMatrix → print → exit
        │
        ▼
desktop-ui/scripts/verify-matrix/matrix.ts      loadManifest · selectChecks · runMatrix
        │                       ▲
        │ spawn("bun run <script>", { cwd, shell, stdio: pipe }) → relay + retain
        ▼                       │
desktop-ui/package.json scripts (unchanged)     scripts/verify-frontend.manifest.json
```

### Manifest

Satisfies: FVM-1.6, FVM-2.1, FVM-2.2
Reuse: none — new code (rung 7); no registry of checks exists anywhere in the repo (fresh retrieval: no owner)
Interface: a JSON array; each entry is `{ "name": string, "command": string, "cwd": string, "mode": "hard" | "report", "profiles": string[] }`. `cwd` is relative to the repository root. Consumers know only these five fields and that order is run order.
Depth: if the manifest vanished, callers would need to know the six package-script names, that they run from `desktop-ui/`, that all are hard, and that they belong to `default` — five fields per row, nothing else.
Locality: new file `scripts/verify-frontend.manifest.json`; `scripts/check-design-tokens.sh` and `scripts/run_chat_perf_gates.sh` — leave (they are commands the manifest points at, not readers of it).

Initial content, in run order: `typecheck`, `lint`, `test`, `check:tokens`, `check:performance`, `test:e2e`, each `command: "bun run <name>"`, `cwd: "desktop-ui"`, `mode: "hard"`, `profiles: ["default"]`. `check:tokens` runs the package script's default mode, which keeps the gate's internal soft/hard split (FVM-1.6); `check:tokens:hard` is not registered.

### Runner core — `matrix.ts`

Satisfies: FVM-1.2, FVM-1.3, FVM-1.4, FVM-1.5, FVM-1.7, FVM-1.8, FVM-1.10, FVM-2.3, FVM-2.4, FVM-2.5, FVM-7.2
Reuse: rung 3 — `node:child_process` `spawn` (Bun implements it; no dependency); rung 2 — the run-all-then-fail shape of `scripts/run_chat_perf_gates.sh:129-134` as the reference pattern
Interface:
- `loadManifest(path: string): ManifestEntry[]` — parses and validates; throws `ManifestError(path, reason)` on unreadable, unparsable, or an entry missing `name`, `command`, `cwd`, `mode`, or a non-empty `profiles` (FVM-2.5).
- `selectChecks(entries, { names?: string[]; profile?: string }): ManifestEntry[]` — no selector → profile `default`; names → those entries in manifest order; profile → entries whose `profiles` include it; an unknown name or profile throws `SelectionError(registeredNames, registeredProfiles)` (FVM-1.10).
- `runMatrix(entries, { repoRoot, spawn?, relay? }): Promise<{ rows: Row[]; exitCode: 0 | 1 }>` — runs each entry in order, awaiting each before the next, via `spawn(entry.command, { cwd: join(repoRoot, entry.cwd), shell: true, stdio: ["inherit", "pipe", "pipe"] })`; every stdout/stderr chunk is written through to `relay` (default: the process's own stdout/stderr) as it arrives and appended to the row's `output`. `Row = { name, mode, result: "pass" | "fail", exitStatus: number | null, durationSec, output: string, launchError?: string }`. A `hard` row with non-zero or null status sets `exitCode = 1` but the loop continues (FVM-1.4); a `report` row never changes `exitCode` (FVM-1.5); a spawn `error` event (no exit status) is a `fail` row that sets `exitCode = 1` regardless of mode (FVM-2.3).
- `formatSummary(rows): string` — one aligned table: name · mode · result · exit · seconds (FVM-1.2).
Depth: if this module vanished, callers would need to know "run each selected entry's command in its cwd, in order, pass exit status through, fail the run only after all have run, and print one table" — one sentence, far smaller than the argv handling, validation messages, and formatting it hides.
Locality: new files under `desktop-ui/scripts/verify-matrix/`; `desktop-ui/scripts/check-performance-budget.sh` — leave; `desktop-ui/vitest.config.ts` — leave (its default glob already includes `desktop-ui/scripts/**/*.test.ts`); tests set `// @vitest-environment node` per file because the project default is jsdom.

The `spawn` and `relay` injection points are what the contract tests use: they pass a temporary manifest whose commands are `sh -c 'exit 0'`, `sh -c 'exit 1'`, and `sh -c 'echo findings; exit 1'` under `report`, collect the relay into a buffer, and assert rows (including `output` containing `findings`), exit code, and that repeated runs yield identical summaries (FVM-7.2). The runner prints a one-line header (`▶ <name>  (<mode>)`) before each check, relays the check's own output verbatim while it runs, and prints nothing else until the summary (FVM-1.3).

### CLI entry — `main.ts` and the root script

Satisfies: FVM-1.1, FVM-1.9
Reuse: rung 6 — one root `package.json` script line; rung 7 for `main.ts` (argv parsing is ~30 lines, no dependency justified)
Surface: root `package.json` `scripts` — new key `verify:frontend`; no existing key changes; readers: humans and `docs/agents/project.md` → `replace` (the table row is added in the Docs module).
Interface: `bun run verify:frontend` runs `default`; positional names select checks; `--profile <name>` selects a profile; `--list` prints name · mode · profiles · cwd · command for every entry and exits 0 without running; `--manifest <path>` overrides the manifest location (used by tests and acceptance). Exit code is `runMatrix`'s `exitCode`, or 1 on `ManifestError` / `SelectionError` after printing the error (FVM-1.10, FVM-2.5).
Depth: if `main.ts` vanished, callers would need to know the three flags, that positional arguments are check names, and that the manifest defaults to `scripts/verify-frontend.manifest.json` under the repo root resolved from `import.meta.dir` — the flag list, nothing more.
Locality: new `desktop-ui/scripts/verify-matrix/main.ts`; root `package.json` — extend by one line; `desktop-ui/package.json` — leave (FVM-5.1).

### CI job

Satisfies: FVM-4.1, FVM-4.2, FVM-4.3, FVM-4.4
Reuse: rung 2 — extend the existing `desktop-ui-quality` job in `.github/workflows/ci.yml:50-74`; rung 4 — `actions/setup-bun`, Homebrew, and `playwright install` are platform features already available on the runner
Surface: `desktop-ui-quality` job — job name unchanged, steps replaced; readers: any GitHub required-status-check rule keyed on the job name (none found in-repo; branch protection is outside the tree) → `replace`, job name preserved so such a rule keeps matching; `rust-quality` and `desktop-build-check` jobs → `frozen` (FVM-4.4, FVM-6.4): the design leaves them untouched.
Interface: after checkout, Bun setup, and `bun install --frozen-lockfile` in `desktop-ui`, three new steps: `command -v rg || brew install ripgrep`; `bunx playwright install chromium` in `desktop-ui`; `bun run verify:frontend` at repo root. The three former steps (Lint, Type check, Test) are removed. The job's wall-clock duration is what GitHub already records per job (FVM-4.4).
Depth: n/a — extends `.github/workflows/ci.yml`
Locality: `.github/workflows/ci.yml` lines 64-74 — replace; lines 14-48 and 76-151 — leave; `release*.yml` — leave.

Weighed, not decided here: caching the Playwright browser with `actions/cache` keyed on the `@playwright/test` version; deferred until the job's measured duration says it matters (Open Question in requirements).

### Docs

Satisfies: FVM-3.1, FVM-3.2, FVM-3.3
Reuse: rung 2 — extend `docs/agents/project.md` (`## verify commands`, lines 76-85) and `docs/standards/desktop-ui.md` (`## Scripts`, `## Done gates`)
Surface: `docs/agents/project.md` verify table — readers: `prove-claim`, `cut-release`, `land-branch`, `validate-*` skills that run "all verify commands" → `replace`: the table gains a `Frontend` row `bun run verify:frontend` and keeps every Rust command and `./scripts/run_chat_perf_gates.sh` with their semantics; the mixed rows may be split into Rust and Frontend rows (FVM-3.3 allows restructuring).
Interface: project.md row: `| Frontend | bun run verify:frontend |` with a one-line note that `bun run verify:frontend --list` shows the registered checks. desktop-ui.md: a `## Verify matrix` section between `## Scripts` and `## Path aliases` describing the entry command, the manifest path and fields, `hard` versus `report`, profiles, and `--list` as discovery only; `## Done gates` points at the matrix as the completion-claim command.
Depth: n/a — extends existing docs
Locality: `docs/agents/project.md` — extend; `docs/standards/desktop-ui.md` — extend; `CLAUDE.md` `## Desktop UI (desktop-ui/)` block (lines 30-39, where the frontend commands are listed) — extend with one line naming `bun run verify:frontend`; the `## Build & Test` block is Rust-only and is left alone; `desktop-ui/AGENTS.md` — leave (points at desktop-ui.md).

### Acceptance script

Satisfies: FVM-5.2, FVM-5.3, FVM-5.4, FVM-5.5, FVM-7.1, FVM-7.3
Reuse: rung 2 — the hand-rolled bash test pattern of `scripts/adapt_codex_vendor.sh.test.sh` (`mktemp`, run, assert with `test`/`grep`, `set -e`)
Interface: `desktop-ui/scripts/verify-matrix/acceptance.sh`, run once during the feature's review from the repo root. For each of the six checks it runs the package script directly in `desktop-ui/` and via `bun run verify:frontend <name>` and asserts equal exit statuses (FVM-5.2); it runs `bun run verify:frontend` twice and diffs the two summaries (FVM-7.3); it records the four quick-check durations from the summary against the 120 s ceiling (FVM-7.1). FVM-5.3, 5.4, 5.5 hold structurally (the runner invokes the unchanged package scripts) and are observed in the same run: `check:tokens` prints its soft/hard sections, `test:e2e` passes the theme smoke, `check:performance` prints its budget lines.
Depth: n/a — follows an existing pattern; the script is a list of assertions, not a module
Locality: new file beside the runner; nothing else changes. It is deliberately not registered in the manifest and not run by CI (Decision 8).

### Change-boundary verification (infrastructure, no Satisfies of its own)

FVM-5.1, FVM-6.1, FVM-6.2, FVM-6.3, FVM-6.4 are review-time diffs, not code: `git diff --exit-code <approved-base> -- <path>` for the token directory, the token gate, the Playwright config and tests, the perf-budget script, and the two Rust CI job blocks (extracted with `sed -n` line ranges and diffed). The approved base is the merge-base with `main` recorded in `tasks.md`. `plan-tasks` turns these into one verification task; no module is built for them.

## Seams for testing

| Seam | Kind | Covers |
|---|---|---|
| `loadManifest(path)` with temp files | unit (vitest, node env) | FVM-2.1, FVM-2.5 |
| `selectChecks(entries, selector)` | unit | FVM-1.5, FVM-1.7, FVM-1.8, FVM-1.10, FVM-2.2 |
| `runMatrix(entries, { repoRoot, spawn, relay })` with fake commands | unit | FVM-1.2, FVM-1.3, FVM-1.4, FVM-1.5, FVM-2.3, FVM-2.4, FVM-7.2 |
| `main.ts` argv → exit code, via `spawnSync("bun", ["main.ts", …])` with `--manifest` temp | integration (vitest, node env) | FVM-1.1 (selection of the six by name from the real manifest, run skipped by `--list`), FVM-1.9, FVM-1.6, FVM-2.2 |
| `acceptance.sh` on an unchanged tree | e2e / acceptance, once | FVM-1.1 (the six run end to end as one unit with one exit code; the `main.ts` row proves only their selection), FVM-5.2, FVM-5.3, FVM-5.4, FVM-5.5, FVM-7.1, FVM-7.3 |
| `.github/workflows/ci.yml` on the feature PR | integration (CI run) | FVM-4.1, FVM-4.2, FVM-4.3, FVM-4.4 |
| review-time `git diff --exit-code` | acceptance | FVM-5.1, FVM-6.1, FVM-6.2, FVM-6.3, FVM-6.4 |
| doc review of `project.md` / `desktop-ui.md` | manual check | FVM-3.1, FVM-3.2, FVM-3.3 |

One new seam is introduced: the `spawn` / `relay` injection on `runMatrix`. Every other row is an existing boundary (files on disk, argv and exit codes, CI, docs).

## Coverage check

Every requirement ID appears in exactly one `Satisfies:` line above:

- Manifest: 1.6, 2.1, 2.2
- Runner core: 1.2, 1.3, 1.4, 1.5, 1.7, 1.8, 1.10, 2.3, 2.4, 2.5, 7.2
- CLI entry: 1.1, 1.9
- CI job: 4.1, 4.2, 4.3, 4.4
- Docs: 3.1, 3.2, 3.3
- Acceptance script: 5.2, 5.3, 5.4, 5.5, 7.1, 7.3
- Deliberately unmapped to a module (review-time diffs, no code): 4.5, 5.1, 6.1, 6.2, 6.3, 6.4 — covered by the change-boundary verification row of the seams table.

35 IDs in requirements.md; 29 mapped to modules, 6 mapped to the review-time diff seam.
