# Desktop UI platform (KlyntBot)

Status: Approved  
Date: 2026-09-02

## Purpose and boundary

Short platform map for `desktop-ui/` — scripts, aliases, where UI lives, IPC, and
glass. Detailed conventions and done gates live in [`ui.md`](./ui.md). Tokens:
[`design-tokens.md`](./design-tokens.md). Perf budgets:
[`frontend-performance.md`](./frontend-performance.md).

## Scripts

Run from `desktop-ui/` (root `package.json` delegates the same names where noted).

| Script | Command | Notes |
|---|---|---|
| Dev server | `bun run dev` | Vite on port 1420 |
| Typecheck | `bun run typecheck` | `tsc --noEmit` |
| Lint | `bun run lint` | Biome check on `src/` |
| Test | `bun run test` | Vitest once |
| Build | `bun run build` | `vite build` |
| Build + types | `bun run build:check` | `tsc --noEmit && vite build` |
| Token gate | `bun run check:tokens` | Design-token hard/soft gate |
| Perf budget | `bun run check:performance` | Build + `scripts/check-performance-budget.sh` |

Root shortcuts: `bun run typecheck` / `lint` / `test` / `build` / `check:tokens` /
`check:performance` from the repo root.

## Path aliases

Configured in `vite.config.ts` + `tsconfig.json`. Always use these — no deep
`../../` imports.

| Alias | Resolves to |
|---|---|
| `@/*` | `src/*` |
| `@shared/*` | `src/shared/*` |
| `@features/*` | `src/features/*` |
| `@app/*` | `src/app/*` |

## DS vs `@shared/ui` vs features

| Layer | Path | Owns |
|---|---|---|
| Design system | `packages/design-system/` | `--ds-*` tokens, recipes, barrel primitives |
| Shared UI | `desktop-ui/src/shared/ui/` (+ `composites/`) | Cross-feature controls / patterns; may re-export DS |
| Features | `desktop-ui/src/features/<domain>/` | Product pages and domain-only components |

Import UI from the **`@klyntbot/design-system` barrel** or **`@shared/ui` /
`@shared/composites`**. No deep `@klyntbot/design-system/...` imports (CSS entry
`styles/index.css` is the exception). Promote reusable, domain-free controls into
the DS (or shared) rather than copying across features.

## IPC hooks

Prefer shared hooks in `@shared/hooks`:

- `useQuery` / `useMutation` / `ipc` — cached reads and mutations
- Feature event hooks (e.g. `useEvent`) for Tauri events

Low-level `invoke()` from `@tauri-apps/api/core` is fine for one-off calls that
do not need caching. Do not invent per-feature `useEffect` + `invoke` waterfalls
for list/detail reads — see [`frontend-performance.md`](./frontend-performance.md).

## Glass

- Prefer DS recipes for new chrome: `island`, `liquid-glass`, `glass`, `capsule`.
- Do not approximate with bare `backdrop-blur`.
- Legacy `glass-*` class names for HUD / tray / overlay live in DS
  `legacy-glass.css` — migration debt; do not add new legacy classes.

## Done gates

Before calling UI work done, follow the checklist in [`ui.md`](./ui.md)
(typecheck, lint, tests for pure logic, light + dark pass, `check:tokens` when
touching colors/chrome). Perf-sensitive shell or dependency changes also run
`bun run check:performance`.

## Related

- [`ui.md`](./ui.md) — conventions + done gates
- [`design-tokens.md`](./design-tokens.md) — token contract
- [`frontend-performance.md`](./frontend-performance.md) — budgets / splitting
- [`desktop-ui/AGENTS.md`](../../desktop-ui/AGENTS.md) — agent pointer
