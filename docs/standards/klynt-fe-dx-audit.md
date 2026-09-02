# Klynt FE design-system + DX audit (portable to jdbot)

Status: Analysis  
Date: 2026-09-02  
Source: `/Users/jayden/Developer/klynt/klynt`

Lens: excellence worth copying for the Tauri sibling app — not a full inventory dump.

## A. Design system maturity

### Token architecture

- Single raw-value vault: `packages/design-system/src/tokens/`
- Canonical `--ds-*`; migration aliases `--lg-*`; no third palette
- `@theme inline` → utilities (`text-fg`, `bg-glass`, `rounded-panel`, `text-ui*`)
- Theme via `html[data-theme="dark"]` + `@custom-variant dark`
- Recipes as `@utility` (`glass`, `island`, `capsule`, …) — ban bare `backdrop-blur`
- `cn()` registers DS font-size tokens with `extendTailwindMerge` (load-bearing)

Contract SSOT: `docs/standards/design-tokens.md`

### Component layers

```
tokens → primitives → composites → patterns
```

Mandatory folder shape: `Component.tsx` + `.types.ts` + `.test.tsx` + `index.ts`  
Barrel-only public API. Domain types banned in the package. Storybook deferred.

Portable members: Field, Button, SegmentedControl, SearchField, ConfirmDialog, Overlay, ModalScrim, DataTable, Cluster, AppStates, Sidebar family, Toolbar, slot AppShell.

Shell-specific (skip for jdbot): TrafficLights, Menubar, LockScreen, wallpaper, launchpad.

### Docs that enforce quality

| Doc | Role |
|---|---|
| `design-tokens.md` | What the system is made of |
| `ui.md` | Where UI lives, done gates |
| `frontend-performance.md` | Budgets + splitting policy |
| `frontend/AGENTS.md` | Pointer-only (no forked rules) |

## B. DX excellence

- Bun workspaces, Biome 2.5.9, TS ~7, Vite 8.2, React 19.2, Tailwind 4.3
- Vitest + Testing Library; Playwright e2e
- moon task graph + lefthook (Biome, file-size, perf budget on push)
- Perf budgets: initial JS ≤300 KB gzip, CSS ≤40 KB
- File-size gate: source ≤400 lines, tests ≤600
- CSS contract tests (e.g. glass material sync) without Storybook

## C. What “standard component” means

1. Folder convention + types + colocated tests  
2. Variants via plain maps + `cn` (CVA optional; klynt DS barely uses it)  
3. Recipes for multi-property chrome  
4. One canonical focus-ring string  
5. Pointer-first hit targets (`control-sm` 22 / `control-md` 28)  
6. Non-trivial logic in framework-free modules  

**AppShell rule:** panes separated by gutter + `island`, not doubled hairlines; Toolbar has no plate.

## D. Top 10 portable upgrades for jdbot (ROI)

1. Mandatory component folder + types + grow DS barrel  
2. Promote 5–7 domain-free composites (Field, SegmentedControl, SearchField, ConfirmDialog, Overlay, Toolbar, Sidebar)  
3. Canonical focus-ring; Field owns form a11y  
4. CSS recipe contract tests  
5. Hard-gate `check:tokens` in CI/hooks  
6. Add `ui.md` + thin `desktop-ui` AGENTS pointer  
7. File-size gate on staged FE files  
8. Slot AppShell pattern in DS (keep product router shell in app)  
9. Gzip initial-bundle budget for Vite/Tauri  
10. Root Biome + working `typecheck` / `tsc && vite build`  

### Non-priorities

Astryx, StyleX, OS menubar/traffic lights, moon (unless CI pain), Storybook-first, replacing lucide overnight.
