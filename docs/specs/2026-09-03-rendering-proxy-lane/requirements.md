# Requirements: Rendering proxy lane

Feature code: PERFPROXY
Status: Implemented
Date: 2026-09-03

Roadmap item: ROAD-4 (MILE-2 — Baselines, verification, migration ledger). Build order: after ROAD-25 (desktop-ui lint green). Frame: `.skills/PERFPROXY/close-package.md`; locks `.skills/PERFPROXY/knowns.md`; evidence `.skills/PERFPROXY/spike-repeatability.md`, `probe-raf.md`.

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

Context. Nothing measures the frontend's rendering cost today; `scripts/run_chat_perf_gates.sh` is Rust-only and its one frontend line references a bench that does not exist. The MILE-2 frame defined a two-tier measurement: Tier 1, this lane, a frequent Playwright WebKit proxy that is report-only and never a sole ship gate; Tier 2, ROAD-5, the authoritative native lane. A run-spike on 2026-09-03 showed that `requestAnimationFrame` cadence in WebKit is repeatable to its 1 ms timer quantum but blind to compositor load (a Lens-like animated ground plus three blurred panels moved nothing), while screenshot capture latency moved from about 12 ms to about 46 ms under the same load. The frame lock was therefore amended: the lane records rAF cadence, callback gaps, **and** screenshot capture latency (`screenshotCaptureMs`), a baseline-relative, compositing-sensitive proxy that is never called paint time or compositor time. The lane registers into the FVM verify matrix (Implemented) under its own profile, reports one of three outcomes with distinct exit codes, and its reader is the human reviewer of MILE-4's rendering-sensitive branches.

Vocabulary. *Row* — one of six measurement rows: `idle-20`, `idle-200`, `scroll-200`, each in `light` and `dark`. *Identity* — the triple `schemaVersion`, `measurementContractVersion`, `environmentKey`. *Baseline* — a committed JSON file under `desktop-ui/tests/perf-proxy/baselines/` for one identity. *Routine run* — a comparison run; it never writes a baseline. *Recalibration* — an explicit run that produces a baseline (local: the tracked file; CI: a candidate artifact).

## 1. Run the six rows locally

**Story:** As a contributor, I want to run `bun run perf:proxy` and see six rows of rAF and screenshot metrics collected from the real chat page in headless WebKit, so that I can measure a rendering change before opening a pull request.

- **PERFPROXY-1.1** WHEN a `bun run perf:proxy` measurement completes successfully THE SYSTEM SHALL have executed exactly the rows `idle-20`, `idle-200`, `scroll-200`, each in `light` and `dark`, serially, in one WebKit browser, from `desktop-ui/playwright.perf-proxy.config.ts`; a row failure may skip the remaining rows, and the run is then `COULD_NOT_MEASURE` (PERFPROXY-2.5), never a partial success.
- **PERFPROXY-1.2** WHEN a row starts THE SYSTEM SHALL create a new browser context with `reducedMotion: "no-preference"`, set the row's theme through page state before navigation, and verify `html[data-theme]` equals the row's theme before any sampling.
- **PERFPROXY-1.3** WHEN a row navigates THE SYSTEM SHALL answer every `/api/*` request from an in-test allowlist that returns deterministic responses for `app_info`, `chat_threads`, `chat_messages`, `view_clear_active`, `journey_milestones`, `journey_item_count`, `flashcard_total_due`, `autotuner_status`, and `autotuner_get_toast_count`, with message ids, timestamps, and content fixed per row.
- **PERFPROXY-1.4** IF the page issues an `/api/*` command that is not in the allowlist THEN THE SYSTEM SHALL fail the row with the command name in the failure message.
- **PERFPROXY-1.5** WHEN a `20` row has navigated THE SYSTEM SHALL verify that the fixed content of every one of the 20 fixture messages is present in the rendered DOM before sampling (the plain `MessageList` path renders all messages; assistant turns carry no class or attribute today, so the check is content-based).
- **PERFPROXY-1.6** WHEN a `200` row has navigated THE SYSTEM SHALL verify that the virtualized rendering path is active and that the last fixture message is rendered before sampling, without requiring all 200 messages to exist in the DOM at once.
- **PERFPROXY-1.7** WHEN the precondition of a row holds THE SYSTEM SHALL wait exactly 600 ms, collect and discard exactly 60 rAF gaps, then collect the row's rAF sample.
- **PERFPROXY-1.8** WHILE a `scroll-200` row samples THE SYSTEM SHALL drive a deterministic, non-smooth scroll trajectory from the bottom of the message list toward the top, with row-mount animations left active.
- **PERFPROXY-1.9** WHEN a row's rAF sample is complete THE SYSTEM SHALL wait 600 ms, then capture K screenshots with `animations: "allow"` and fixed, recorded screenshot options, timing each capture end to end.
- **PERFPROXY-1.10** WHEN a row completes THE SYSTEM SHALL record rAF `sampleCount`, `p50Ms`, `p95Ms`, `maxMs` and screenshot `captureCount`, `p50Ms`, plus the screenshot options used.
- **PERFPROXY-1.11** THE SYSTEM SHALL inject no CSS or script that disables animations, and SHALL leave production source under `desktop-ui/src` free of any fixture mode or test-only transport branch; a neutral, always-present DOM attribute that names the rendering path is not a fixture mode and may be added if the design chooses it.
- **PERFPROXY-1.12** WHEN all rows have completed THE SYSTEM SHALL write `desktop-ui/test-results/perf-proxy/latest.json` containing the identity, full environment metadata, every row's metrics, and the run's outcome.

## 2. Compare against the baseline

**Story:** As a contributor, I want the run to end with one of `HEALTHY`, `DEGRADED`, or `COULD_NOT_MEASURE` and a matching exit code, so that the matrix line tells me what happened without opening logs.

- **PERFPROXY-2.1** WHEN the Playwright child exits zero THE SYSTEM SHALL read `latest.json`, locate the baseline whose identity equals the run's identity, and compare each row's rAF `p95Ms` and screenshot `p50Ms` against that baseline using the per-metric margins stored in the baseline.
- **PERFPROXY-2.2** WHEN every compared metric is within its margin THE SYSTEM SHALL print `HEALTHY` with the environment identity and per-row deltas and exit 0.
- **PERFPROXY-2.3** WHEN at least one compared metric exceeds its margin THE SYSTEM SHALL print `DEGRADED` naming each affected row, metric, baseline value, current value, and delta, and exit 1.
- **PERFPROXY-2.4** IF no baseline matches the run's identity THEN THE SYSTEM SHALL print `COULD_NOT_MEASURE` with subtype `NO_BASELINE` and the identity that was not found, and exit 2.
- **PERFPROXY-2.5** IF a row's precondition fails, the allowlist is violated, the browser or web server or a test fails to launch, a row times out, the identity fields are malformed, `latest.json` is missing or malformed, or the Playwright child exits non-zero THEN THE SYSTEM SHALL print `COULD_NOT_MEASURE` with a subcode naming the cause and exit 2.
- **PERFPROXY-2.6** THE SYSTEM SHALL never exit 0 unless a baseline matched and every row completed with a trustworthy measurement.
- **PERFPROXY-2.7** WHEN the wrapper cannot write `latest.json` THE SYSTEM SHALL still print `COULD_NOT_MEASURE` and exit 2.
- **PERFPROXY-2.8** IF port 1420 is already occupied when the perf configuration starts its web server THEN THE SYSTEM SHALL refuse to measure the foreign server, print the port in the message, and exit 2.

## 3. Recalibrate explicitly

**Story:** As the tech lead, I want to record a new baseline only when I ask for it, so that a regression can never quietly become the new normal.

- **PERFPROXY-3.1** WHEN the lane's recalibration entry point runs THE SYSTEM SHALL execute R repetitions of all six rows and produce a baseline document containing per-row statistics, repetition count, per-metric noise data and margins, the source revision, the recording time, the identity, and full environment metadata.
- **PERFPROXY-3.2** WHEN recalibration runs locally THE SYSTEM SHALL write the baseline to `desktop-ui/tests/perf-proxy/baselines/<environmentKey>.json`, so that the change appears as a reviewable diff.
- **PERFPROXY-3.3** WHEN recalibration runs in CI THE SYSTEM SHALL upload the baseline as a workflow artifact and SHALL NOT commit, push, or replace any tracked file.
- **PERFPROXY-3.4** THE SYSTEM SHALL provide no routine-run flag or argument that writes a baseline; recalibration is reachable only through its own entry point.
- **PERFPROXY-3.5** THE SYSTEM SHALL derive `environmentKey` from stable comparison dimensions only (runner class, platform, architecture, OS major, WebKit revision, headless mode, viewport, device pixel ratio) and record other environment facts as diagnostic metadata outside the key. On a developer machine the runner class is `local` plus the full SHA-256 digest of the hostname as canonical identity input, truncating only the final `environmentKey`; this prevents accidental baseline sharing between machines with different hostnames (it does not guarantee uniqueness across identical hostnames). The raw hostname SHALL never appear in tracked baselines, candidate artifacts, filenames, or serialized evidence — verified by a test that scans every file the lane writes for the hostname string.

## 4. CI runs the lane on relevant changes

**Story:** As a reviewer of a MILE-4 branch, I want the rendering-proxy workflow's outcome, identity, and per-row deltas in the job summary of every relevant push or pull request, so that I can record a disposition without running anything.

- **PERFPROXY-4.1** WHEN a push or pull request targeting `main` or `dev` changes any path under `desktop-ui/`, `packages/design-system/`, `scripts/verify-frontend.manifest.json`, or the rendering-proxy workflow file THE SYSTEM SHALL run a dedicated `rendering-proxy` workflow, using GitHub Actions' native path filters and no changed-files action or shell diff.
- **PERFPROXY-4.2** WHEN the workflow runs THE SYSTEM SHALL install the WebKit browser required by the perf configuration before invoking `bun run perf:proxy`.
- **PERFPROXY-4.3** WHEN the routine run finishes with any outcome THE SYSTEM SHALL write the terminal outcome, the environment identity, the per-row deltas, and the sentence "This result is an advisory WebKit proxy, not native rendering evidence." to `$GITHUB_STEP_SUMMARY`.
- **PERFPROXY-4.4** WHEN `latest.json` exists after the run, including `DEGRADED` and `COULD_NOT_MEASURE` runs THE SYSTEM SHALL upload it as a workflow artifact.
- **PERFPROXY-4.5** WHEN the workflow is dispatched manually with the recalibration input THE SYSTEM SHALL run the recalibration entry point directly and upload the candidate baseline as an artifact (PERFPROXY-3.3).
- **PERFPROXY-4.6** THE SYSTEM SHALL NOT configure the rendering-proxy workflow as a required branch-protection check — verified at acceptance by a human reading the repository's branch-protection settings for `main` and `dev` (or `gh api repos/{owner}/{repo}/branches/{branch}/protection`) and recording the result in the review; a workflow run or PR description does not prove it.
- **PERFPROXY-4.7** WHEN the workflow job completes THE SYSTEM SHALL leave its wall-clock duration readable in the job log, so that browser caching and cadence decisions can cite it.

## 5. Registered in the matrix, isolated from the smoke

**Story:** As a contributor, I want the lane listed by the verify matrix under its own profile and invisible to the chromium smoke, so that neither run changes what the other means.

- **PERFPROXY-5.1** WHEN this feature lands THE SYSTEM SHALL add exactly one entry to `scripts/verify-frontend.manifest.json` with `command: "bun run perf:proxy"`, `cwd: "desktop-ui"`, `mode: "report"`, `profiles: ["perf-proxy"]`.
- **PERFPROXY-5.2** WHEN `bun run verify:frontend` runs with no arguments THE SYSTEM SHALL NOT launch WebKit or run any perf row.
- **PERFPROXY-5.3** WHEN `bun run verify:frontend --profile perf-proxy` runs THE SYSTEM SHALL run the lane and show its outcome as `pass 0`, `fail 1`, or `fail 2` in the matrix summary without changing the matrix exit code.
- **PERFPROXY-5.4** THE SYSTEM SHALL keep the perf configuration's `testDir` at `desktop-ui/tests/perf-proxy` and match only `**/*.perf.ts`, with one WebKit project, `fullyParallel: false`, `workers: 1`, `retries: 0`, tracing off, `reuseExistingServer: false`, and its own Vite `webServer`.
- **PERFPROXY-5.5** WHEN `bun run test:e2e -- --list` runs THE SYSTEM SHALL list the chromium theme smoke and no perf test.
- **PERFPROXY-5.6** WHEN `bun run perf:proxy -- --list` runs THE SYSTEM SHALL list only the WebKit perf rows and no smoke test.
- **PERFPROXY-5.7** (guard) WHEN this feature lands THE SYSTEM SHALL CONTINUE TO ship `desktop-ui/playwright.config.ts`, the `test:e2e` package script, and `desktop-ui/tests/smoke/theme-toggle.spec.ts` unchanged — verified by `git diff --exit-code <approved-base>` on those paths.
- **PERFPROXY-5.8** (guard) WHEN this feature lands THE SYSTEM SHALL CONTINUE TO ship `desktop-ui/scripts/verify-matrix/` unchanged and the six existing manifest entries byte-for-byte unchanged.

## 6. The disposition protocol is written down

**Story:** As the author of a MILE-4 spec, I want the advisory-disposition protocol in the frontend performance standard, so that ROAD-12, ROAD-13, and ROAD-14 can cite it instead of restating it.

- **PERFPROXY-6.1** WHEN this feature lands THE SYSTEM SHALL add a section to `docs/standards/frontend-performance.md` that names the lane as an advisory WebKit proxy, states that ROAD-5 native evidence alone decides the ambient-ground default, and defines the required human record for each outcome: `HEALTHY` (outcome and run or artifact reference), `DEGRADED` (affected rows and deltas, disposition, reason for proceeding or investigating), `COULD_NOT_MEASURE` (cause, coverage gap, follow-up).
- **PERFPROXY-6.2** WHEN this feature lands THE SYSTEM SHALL state in the same section that recording a disposition is required for MILE-4 rendering-sensitive branches and that obtaining `HEALTHY` is not a shipping condition.
- **PERFPROXY-6.3** WHEN this feature lands THE SYSTEM SHALL describe the manifest entry, the `perf-proxy` profile, the local commands, the outcome codes, and the two-step baseline bootstrap in the `## Verify matrix` section of `docs/standards/desktop-ui.md`, and update the browser E2E line of `docs/agents/project.md` so it no longer implies Chromium is the only Playwright engine in use.
- **PERFPROXY-6.4** THE SYSTEM SHALL document `latest.json` as a versioned evidence format that MILE-4 specs reference and do not parse as a machine API.

## 7. Quality attributes

**Section-kind:** nfr

**Story:** As a stakeholder, I want measurable quality targets for this feature, so that how-well is not left implicit.

- **Performance:** **PERFPROXY-7.1** WHEN `bun run perf:proxy` runs a routine run on the machine that records the local baseline THE SYSTEM SHALL complete all six rows within 60 seconds excluding browser launch — verified by the run's own duration output during the final repeatability spike (the 2026-09-03 spike measured 31 s). Consult: `docs/product/metrics.md` absent, `docs/ops/reliability.md` absent; the number comes from the frame's weigh, no standing metric ID.
- **Security:** **PERFPROXY-7.2** THE SYSTEM SHALL reach no network endpoint other than the lane's own Vite server on localhost, read no secret, and grant the rendering-proxy workflow only the permissions needed to read the checkout and upload artifacts — verified by review of the workflow's `permissions` block and the route allowlist. Consult: `docs/security/threat-model.md` absent; domain judgment, no standing ID.
- **Reliability:** **PERFPROXY-7.3** WHEN five consecutive routine runs execute on an unchanged tree against a matching baseline on the same environment THE SYSTEM SHALL report `HEALTHY` on all five — verified during the final repeatability spike and recorded in the review. **PERFPROXY-7.4** WHEN a Lens-like load (an animated blurred ground and three `backdrop-filter` panels) is injected into an otherwise unchanged tree THE SYSTEM SHALL report `DEGRADED` on the idle rows using only the recorded baseline and margins — verified once during the final spike and recorded in the review. Consult: `docs/ops/reliability.md` absent; targets come from the close package's success signal.
- **Accessibility:** None — headless measurement lane with no user interface.

## Out of Scope

- Live SSE streaming, a real backend, or any fixture code in production source.
- Rows for resize, composer typing, sidebar interaction, theme flips during sampling, or reduced motion; a reduced-motion diagnostic may be added later outside the six rows.
- Joining the `default` profile; hardening the lane; making it a required check; any automated consumer of `latest.json`; gating merges or the ambient-ground default.
- The native production-runtime lane (ROAD-5) and its cadence.
- Editing the chromium smoke, `desktop-ui/playwright.config.ts`, or FVM's runner.
- Fixing the `AmbientIndicator` crash on a truthy `autotuner_status` without `champion`; that is a separate `root-cause` item.
- `desktop-ui.new-bak/`.

## Open Questions

- Final repeatability spike: R repetitions, K captures, rAF and screenshot margins, long-gap definition, runtime proof — this feature's design and build — before recalibration lands — forbid-guess (cấm đoán).
- Exact screenshot options (viewport or clip, scale, format, in-memory) — this feature's design — at design — forbid-guess (cấm đoán).
- Fixture type-check mechanism (Playwright execution alone does not type-check fixture builders) — this feature's design — at design — forbid-guess (cấm đoán).
- Virtualized-path signal: no existing attribute distinguishes the paths (both roots carry `aria-live="polite"`, react-window's outer element has no `data-*`); sentinel-based detection or a neutral `data-render-path` attribute — this feature's design — at design — forbid-guess (cấm đoán).
- Artifact name and retention; banner text and `COULD_NOT_MEASURE` subcodes; where the disposition lives in the task report — this feature's design — at design — forbid-guess (cấm đoán).
- Final `environmentKey` field list; runner-image revision stays diagnostic unless the spike proves it enters the key — this feature's design, informed by the spike — at design — forbid-guess (cấm đoán).
- Explicit human ownership of PERFPROXY — unassigned pending explicit assignment — open — forbid-guess (cấm đoán).
