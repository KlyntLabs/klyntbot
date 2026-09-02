# Frontend performance standards (KlyntBot desktop)

Status: Approved  
Date: 2026-09-02

## Purpose and boundary

Keep the Tauri desktop UI fast — cold start of the webview, route transitions, and
interaction on chrome — as a build-time concern. Applies to `desktop-ui/src/**`.
UI conventions live in [`ui.md`](./ui.md); this file does not redefine tokens.

This is a **desktop app**, not a public web site: Lighthouse-first scoring is not
the north star. Prefer local timing, bundle analysis, and React profiler traces.
**Structured observability dashboards (OpenTelemetry / Prometheus) are a product
non-goal** — do not add them for FE perf.

**Enforcement:** `desktop-ui/scripts/check-performance-budget.sh` is the
executable source of truth for budget numbers. Run via
`bun run check:performance` from `desktop-ui/` (or the repo root). Budgets may
be **tightened** in a normal PR; **loosening one requires explicit owner
approval** in the PR description.

## Budgets (current values, mirrored from the script)

| Budget | Limit (gzipped) | Notes |
|---|---|---|
| Entry JS (`<script>` tags in `dist/index.html`, fallback `main-*.js`) | 800 kB | Soft Tauri-local budget; calibrated 2026-09-02 (~44 kB today) |
| Main CSS (`main-*.css` or largest CSS) | 80 kB | Token + chrome CSS; calibrated 2026-09-02 (~23 kB today) |

The script also prints the top 10 JS chunks by gzip size for visibility — those
are not individually gated. Do not raise a budget as the first response to a
regression — split or drop weight first.

## Startup and bundle

| Topic | Rule |
|---|---|
| Route-level lazy load | Feature pages load via `React.lazy` at the router boundary (`src/app/router.tsx`). Keep the initial chunk to shell chrome + shared primitives. |
| Heavy deps | TipTap, Mermaid, Three / force-graph, Recharts stay in Vite `manualChunks` and/or async import with the feature that needs them — never pull into the shell entry. |
| Dependency cost | New runtime deps need ui.md stack approval **and** a stated gzip cost. Prefer React 19 / existing packages. |
| Analysis | When a target is blown, inspect the bundle and split or drop — do not silently inflate the number. |

## Interaction

| Topic | Rule |
|---|---|
| Animation | Animate **transform / opacity only** on persistent chrome (sidebar, panels, HUD windows). Never animate layout props (`top`/`left`/`width`/`height`) on chrome. |
| Glass cost | Prefer the `island` recipe over stacked double `backdrop-blur` surfaces. Nested blurs are expensive in the webview. |
| Long lists | Unbounded lists (messages, search results, task tables, note trees) get virtualization or pagination once real data can exceed ~100 rows. |
| Render discipline | `memo` / `useMemo` only for a **measured** re-render problem (React DevTools profiler), not by default. |
| Pure engines | Non-trivial UI logic stays framework-free (also a perf seam for isolated benchmarks). |

## Data plane (perceived performance)

| Topic | Rule |
|---|---|
| IPC state | Prefer shared `@shared/hooks` `useQuery` / `useMutation` (or feature equivalents) over ad-hoc `useEffect` + `invoke` waterfalls for list/detail reads. |
| No waterfalls | Fire independent IPC reads in parallel on route enter; do not chain request → render → request when avoidable. |
| Loading | Prefer skeletons in-place over blanking a surface that already has content; keep stale data visible while revalidating when safe. |

## Measurement, not vibes

- Perf work and review objections must cite a measurement: build artifact sizes,
  a profiler trace, or a reproducible timing. Speculative micro-optimization is
  rejected in review.
- Changes that touch shell hot paths or add a runtime dependency should run
  `bun run check:performance` locally and note chunk impact before push.
