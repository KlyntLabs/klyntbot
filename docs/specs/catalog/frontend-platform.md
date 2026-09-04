# frontend-platform catalog

Recognized feature cards for this domain. Add a row **before** writing
`requirements.md` for a new feature. Status is one of
`Draft | Approved | In-progress | Implemented | Shipped | Recognized`.

| Code | Feature | Spec | Status | Roadmap item | Match terms | Surface roots |
|---|---|---|---|---|---|---|
| FVM | Frontend verify matrix | ../2026-09-03-frontend-verify-matrix/ | Implemented | ROAD-3 | verify entry point, frontend checks, typecheck lint vitest, token gate report mode, Playwright smoke, perf budget, lane registration, verify matrix docs/agents/project.md | `desktop-ui/package.json`, `scripts/`, `docs/agents/project.md` |
| PERFPROXY | Rendering proxy lane | ../2026-09-03-rendering-proxy-lane/ | Implemented | ROAD-4 | rendering proxy, Playwright WebKit, rAF cadence, screenshot capture latency, perf-proxy profile, baseline recalibration, HEALTHY DEGRADED COULD_NOT_MEASURE, advisory disposition, latest.json | `desktop-ui/tests/perf-proxy/`, `desktop-ui/playwright.perf-proxy.config.ts`, `.github/workflows/rendering-proxy.yml` |
