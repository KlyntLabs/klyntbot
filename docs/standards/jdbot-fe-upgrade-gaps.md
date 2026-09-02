# jdbot desktop-ui — FE upgrade gap analysis (post DS migration)

Status: Analysis  
Date: 2026-09-02  
Companion: [`klynt-fe-dx-audit.md`](./klynt-fe-dx-audit.md), [`design-tokens.md`](./design-tokens.md)

## Executive summary

Token migration landed. Remaining work is **maturity debt**, not a failed migration:

1. DS is tokens + Button — not a full control library  
2. Dual chrome (`glass-*` legacy vs DS recipes; `compat.css`)  
3. DX holes (broken `typecheck`, thin tests, dual lockfiles, Claude.md drift)  
4. Stack mostly modern; notable lags: **Vite 6** (vs 7/8), **lucide 0.487** (vs 1.x)

## Strengths

- `@klyntbot/design-system` tokens, `@theme`, recipes, `cn`, light/dark  
- Feature-sliced app, path aliases, route lazy-load, heavy `manualChunks`  
- React Compiler, Biome, custom IPC query hooks (fit Tauri)  
- Soft/hard token scripts; migration review ship-with-nits  

## Gaps

### Design system

- Only `primitives/button` in package; composites/patterns empty  
- Product still owns Input/Toggle/Dialog/… in `shared/ui`  
- Tasks feature has a second Radix/shadcn island under `features/tasks/components/ui/`  
- `glass.css` still imported (~97+ legacy class usages)  
- `compat.css` + temporary `@theme` aliases still ship  
- Ad-hoc `text-[Npx]` and `z-[9999]` remain in places  

### DX / tooling

- Root `typecheck` calls missing `desktop-ui` script  
- Build is `vite build` only (no `tsc --noEmit` gate)  
- Claude.md stale (ESLint, old aliases)  
- Dual lockfiles (`bun.lock` + `package-lock.json`)  
- No file-size / perf-budget / lefthook FE gates  
- ~14 test files vs huge feature surface; DS has only `cn.test.ts`  

### Versions (Sep 2026)

| Package | jdbot | klynt | Note |
|---|---|---|---|
| React | ^19.0.0 | ^19.2.7 | Low risk |
| Vite | ^6.4.2 | ^8.2.1 | Medium — plan major wave |
| Tailwind | ^4.1.12 | ^4.3.3 | Low |
| Biome | ^2.4.5 | 2.5.9 | Low |
| TypeScript | ^5.7.0 | ~7.0.2 | Evaluate later |
| lucide-react | ^0.487.0 | — | High drift vs 1.x |
| @vitejs/plugin-react | ^4.7.0 | ^6.1.0 | Coupled to Vite |

## Keep intentional

Tauri multi-window HUD, lucide, no Astryx, TipTap/notes stack, custom IPC hooks, Biome, domain prose/editor CSS, external app-brand color maps.

## Upgrade waves

### P0 — before any feature work

1. Fix `typecheck` script; gate CI: lint + typecheck + test + `check:tokens`  
2. Truth docs (Claude.md aliases/lint/hooks)  
3. Bun-only lockfile hygiene  
4. Harden token gate carve-outs; bump Vite 6.4.x + Tauri API minors  

**Exit:** root scripts green; docs match reality.

### P1 — finish design-system adoption

1. Replace `glass-*` with recipes; shrink/delete `glass.css`  
2. Promote Input, Toggle, Checkbox, Badge, Skeleton, Tooltip, Dialog into DS  
3. Collapse tasks Radix island into shared/DS  
4. Typography + z-index sweep  
5. Retire `compat.css` when grep-clean  
6. lucide 1.x dedicated PR  

**Exit:** DS owns core controls; hard token gate clean; light/dark pass.

### P2 — platform maturity

1. Vite 7 (then evaluate 8)  
2. `docs/standards/desktop-ui.md`  
3. DS component tests + Playwright smoke (theme toggle, settings)  
4. a11y pass (focus, `aria-busy`, reduced-motion in recipes)  
5. Bundle size budgets  
6. TS 6 / Vitest 5 after Vite settles  

## Do not do during upgrade

- Reintroduce Astryx / second icon system  
- Rewrite TipTap/force-graph “for cleanliness”  
- Swap IPC hooks → TanStack in the same wave as Vite/DS  
- Delete `compat.css` before hard grep proves unused  
