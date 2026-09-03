# Requirements: Frontend verify matrix

Feature code: FVM
Status: In-progress
Date: 2026-09-03

Roadmap item: ROAD-3 (MILE-2 — Baselines, verification, migration ledger). Frame: `.skills/FVM/close-package.md`; locks in `.skills/FVM/knowns.md`. Approved base revision for guards: the `main` merge-base of the feature branch at approval time, recorded in `tasks.md`.

<!--
Rules:
- Feature code: 2-12 chars, A-Z0-9, starts with a letter, unique repo-wide.
  Register it in docs/specs/INDEX.md before use.
- Every acceptance criterion gets a hierarchical ID: <CODE>-<story>.<criterion>.
- Criteria use EARS phrasing:
    WHEN <event/condition> THE SYSTEM SHALL <behavior>          (event-driven)
    WHILE <state> THE SYSTEM SHALL <behavior>                   (state-driven)
    IF <unwanted condition> THEN THE SYSTEM SHALL <behavior>    (unwanted behavior)
    WHERE <feature is included> THE SYSTEM SHALL <behavior>     (optional feature)
    THE SYSTEM SHALL <behavior>                                 (ubiquitous)
- Guard requirements protect existing behavior this feature touches:
    WHEN <condition> THE SYSTEM SHALL CONTINUE TO <existing behavior>
- IDs are immutable once Status is Approved. Retire a requirement by striking it
  through (~~**CODE-N.M**~~ reason) — never renumber.
-->

Context. Today the frontend has six independent checks (`typecheck`, `lint`, `test`, `check:tokens`, `check:performance`, `test:e2e` in `desktop-ui/package.json`). CI's `desktop-ui-quality` job runs only the first three. The token gate, the Playwright smoke, and the performance budget run only when someone remembers. The verify-commands table in `docs/agents/project.md` names `bun run typecheck` and `bun run lint` and nothing else for the frontend. This feature gives the frontend one verify entry point ("the matrix") with a per-check report, one manifest where future checks register with a profile so registration never silently decides when a lane runs, and full six-check parity between the local default run and CI. It changes no visual token and no check's own rules. The matrix captures each check's output, result, duration, and exit status; structured findings and ratchet counters belong to ROAD-6.

## 1. One entry point, one report

**Story:** As a contributor, I want to run one command from the repository root that runs every frontend check and prints a per-check report with a single exit code, so that I can prove the frontend healthy before claiming completion.

- **FVM-1.1** WHEN a contributor runs the matrix with no arguments THE SYSTEM SHALL run the `default` profile: `typecheck`, `lint`, `test`, `check:tokens`, `check:performance`, `test:e2e`, each as its existing `desktop-ui` package script.
- **FVM-1.2** WHEN all selected checks have finished THE SYSTEM SHALL print one summary listing every check with its mode (`hard` or `report`), its result (`pass` or `fail`), its exit status, and its wall-clock duration in seconds.
- **FVM-1.3** WHEN a check runs THE SYSTEM SHALL capture that check's standard output and standard error and make them available in the run output, without parsing or counting their contents.
- **FVM-1.4** IF a `hard` check exits non-zero THEN THE SYSTEM SHALL still run every remaining selected check and exit non-zero only after the summary is printed.
- **FVM-1.5** WHILE a check is declared `report` THE SYSTEM SHALL record its result and exit status in the summary without changing the matrix exit code.
- **FVM-1.6** THE SYSTEM SHALL declare all six initial checks as `hard`; `check:tokens` keeps its own internal policy (legacy utilities, bare variables, and deep imports hard; raw-color findings soft until MILE-5) because the matrix invokes it in default mode.
- **FVM-1.7** WHEN the matrix is invoked with one or more check names THE SYSTEM SHALL run only those checks, in manifest order, and report only those.
- **FVM-1.8** WHEN the matrix is invoked with a profile name THE SYSTEM SHALL run every check whose declared profiles include that name, in manifest order.
- **FVM-1.9** WHEN the matrix is invoked with the `--list` flag THE SYSTEM SHALL print every registered check with its mode, profiles, working directory, and command, and run nothing.
- **FVM-1.10** IF the matrix is invoked with a check or profile name that is not registered THEN THE SYSTEM SHALL print the registered names and exit non-zero without running any check.

## 2. One place to register a check

**Story:** As a contributor adding a measurement lane, I want to register the new check in exactly one declared place with the profile it belongs to, so that the matrix runs and reports it whenever that profile is selected, without anyone editing the runner.

- **FVM-2.1** THE SYSTEM SHALL define the set of checks — name, command, working directory, mode, and profiles — in exactly one manifest file under `scripts/`, and the runner SHALL contain no check-specific branches.
- **FVM-2.2** THE SYSTEM SHALL require every manifest entry to declare at least one profile; an entry whose profiles do not include `default` does not run in a no-argument invocation.
- **FVM-2.3** WHEN a new entry is appended to the manifest THE SYSTEM SHALL run and report it on the next invocation that selects one of its declared profiles, with no other change — verified by a contract test that registers a throwaway check under a temporary manifest.
- **FVM-2.4** IF a manifest entry's command cannot be started THEN THE SYSTEM SHALL report that check as `fail` with the launch error, and treat it as a `hard` failure regardless of its declared mode.
- **FVM-2.5** IF the manifest cannot be parsed, or an entry lacks a name, command, mode, or profile THEN THE SYSTEM SHALL print the error with the manifest path and exit non-zero without running any check.

## 3. The verify table names the frontend

**Story:** As a contributor or skill reading the verify commands, I want the frontend matrix in the verify-commands table as the command to run, so that every completion claim in this repo includes the six frontend checks.

- **FVM-3.1** WHEN this feature lands THE SYSTEM SHALL list the matrix's real no-argument command as its own row in the verify-commands table of `docs/agents/project.md`.
- **FVM-3.2** WHEN this feature lands THE SYSTEM SHALL document `--list` in `docs/standards/desktop-ui.md` as the registry-discovery command, together with the manifest, profiles, and the `hard` / `report` modes; `--list` is never presented as a verification command.
- **FVM-3.3** (guard) WHEN the verify-commands table is edited THE SYSTEM SHALL CONTINUE TO name every existing Rust command (`cargo check --workspace`, `cargo clippy --workspace --all-targets --all-features`, `cargo fmt --all --check`, `cargo nextest run --workspace`) and `./scripts/run_chat_perf_gates.sh` with unchanged semantics; the rows may be restructured.

## 4. CI runs the same entry point

**Story:** As a contributor, I want CI's frontend job to call the same matrix I run locally with the same six checks, so that local and CI never disagree about what "frontend passes" means.

- **FVM-4.1** WHEN the `desktop-ui-quality` CI job runs THE SYSTEM SHALL invoke the matrix's no-argument command in place of its separate lint, type-check, and test steps.
- **FVM-4.2** WHEN CI runs the matrix THE SYSTEM SHALL run the same six `default` checks as a local no-argument run, including `check:performance` and `test:e2e`.
- **FVM-4.3** IF the browser required by the Playwright project is not installed on the CI runner THEN THE SYSTEM SHALL install it in a CI step before the matrix runs, so that `test:e2e` never fails for a missing browser.
- **FVM-4.4** WHEN the CI job completes THE SYSTEM SHALL record the job's wall-clock duration where a later decision on quick-per-PR versus full parity can read it (the job log is sufficient).
- **FVM-4.5** (guard) WHEN the CI workflow is edited THE SYSTEM SHALL CONTINUE TO run the `rust-quality` and `desktop-build-check` jobs with unchanged steps.

## 5. Existing checks behave the same directly or through the matrix

**Story:** As a contributor, I want every existing `desktop-ui` script to behave exactly as before whether I run it directly or through the matrix, so that adopting the matrix changes nothing about what each check means.

- **FVM-5.1** (guard) WHEN any `desktop-ui` package script present at the approved base revision (including `preview`) is run directly THE SYSTEM SHALL CONTINUE TO behave as at that revision — verified by an exit-code diff of the `scripts` block of `desktop-ui/package.json` against the approved base.
- **FVM-5.2** (guard) WHEN the matrix runs a check THE SYSTEM SHALL CONTINUE TO produce the same exit status as running that check's package script directly in `desktop-ui/` — verified for each of the six by a contract test that compares exit statuses.
- **FVM-5.3** (guard) WHEN the matrix runs `check:tokens` THE SYSTEM SHALL CONTINUE TO run `scripts/check-design-tokens.sh` in its default mode; `check:tokens:hard` remains available directly and unchanged.
- **FVM-5.4** (guard) WHEN the matrix runs `test:e2e` THE SYSTEM SHALL CONTINUE TO pass the existing `tests/smoke/theme-toggle.spec.ts` against `bun run dev` on port 1420 with no backend running.
- **FVM-5.5** (guard) WHEN the matrix runs `check:performance` THE SYSTEM SHALL CONTINUE TO produce a production build first and gate on the existing gzipped entry-JS and main-CSS budgets.

## 6. Change boundary

**Story:** As the tech lead, I want proof that this feature moved nothing outside its boundary, so that the baselines MILE-2 records describe the app as it is.

- **FVM-6.1** (guard) THE SYSTEM SHALL CONTINUE TO ship `packages/design-system/src/tokens/` unchanged — verified by `git diff --exit-code <approved-base> -- packages/design-system/src/tokens/` in the feature's review.
- **FVM-6.2** (guard) THE SYSTEM SHALL CONTINUE TO ship `scripts/check-design-tokens.sh`, including its five raw-literal carve-outs and its `HARD` handling, unchanged — verified by `git diff --exit-code <approved-base> -- scripts/check-design-tokens.sh`.
- **FVM-6.3** (guard) THE SYSTEM SHALL CONTINUE TO ship `desktop-ui/playwright.config.ts`, `desktop-ui/tests/`, and `desktop-ui/scripts/check-performance-budget.sh` unchanged — verified by `git diff --exit-code <approved-base>` on those paths.
- **FVM-6.4** (guard) THE SYSTEM SHALL CONTINUE TO run the `rust-quality` and `desktop-build-check` CI jobs unchanged — verified by diffing those job blocks of `.github/workflows/ci.yml` against the approved base.

## 7. Quality attributes

**Section-kind:** nfr

**Story:** As a stakeholder, I want measurable quality targets for this feature, so that how-well is not left implicit.

- **Performance:** **FVM-7.1** WHEN the matrix runs `typecheck`, `lint`, `test`, and `check:tokens` with a warm dependency cache on the machine that records the MILE-2 baselines THE SYSTEM SHALL complete those four within 120 seconds wall clock — verified by the summary durations, recorded once in the feature's review. Consult: `docs/product/metrics.md` absent, `docs/ops/reliability.md` absent; the number is domain judgment, no standing metric ID.
- **Security:** None — local scripts only; the only network activity is the Vite dev server on localhost:1420 that `test:e2e` already starts; no secrets are read.
- **Reliability:** **FVM-7.2** WHEN the runner is exercised by contract tests using a temporary manifest with fake `pass`, `fail`, and `report` commands THE SYSTEM SHALL produce the same summary and exit code on every repetition — verified by those tests in the frontend unit suite. **FVM-7.3** WHEN the matrix is run twice on an unchanged tree once during this feature's acceptance THE SYSTEM SHALL produce the same per-check results and exit code — verified by that single acceptance run, recorded in the review; CI does not repeat the suite on every execution. Consult: `docs/ops/reliability.md` absent; no standing SLO.
- **Accessibility:** None — command-line tool with no user interface.

## Out of Scope

- Any change to `--ds-*` token values, recipes, or classes (MILE-3).
- The rendering proxy lane (ROAD-4) and the native production-runtime lane (ROAD-5), and the cadence at which they run; this feature only provides the manifest and profiles they register into.
- Structured finding counts, a machine-readable findings protocol, the migration ledger, its categories, ratchet counters, and the central exclusion for `desktop-ui.new-bak/` (ROAD-6).
- Hardening `check:tokens`' raw-color check, or changing any token-gate rule or carve-out (MILE-5).
- A `quick` profile or quick-per-PR CI; full parity holds until a measured CI duration justifies revisiting it.
- Running the Rust checks; the matrix is frontend-only and sits beside the cargo commands in the verify table.
- Running against `desktop-ui.new-bak/`; the manifest points at `desktop-ui/` only.

## Open Questions

- Explicit human ownership of FVM — unassigned pending explicit assignment — open — forbid-guess (cấm đoán).
- The measured CI duration that would justify revisiting full parity — unassigned pending explicit assignment — after the first CI run of the matrix — forbid-guess (cấm đoán).
