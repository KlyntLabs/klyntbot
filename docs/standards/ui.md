# UI standards (KlyntBot desktop)

Status: Approved  
Date: 2026-09-02

## Purpose and boundary

Conventions for the Tauri desktop UI so chrome and feature screens stay consistent
with `@klyntbot/design-system`, Liquid Glass tokens, and feature-sliced layout.

Applies to `desktop-ui/src/**` and `packages/design-system/**`. Token vocabulary,
scales, and forbidden patterns live in [`design-tokens.md`](./design-tokens.md).
This file covers where UI lives, how to consume it, and done gates.

**Out of scope:** Astryx, Storybook (not required), OS-shell chrome (traffic
lights / menubar / wallpaper — klynt-only), and OpenTelemetry UI dashboards.

## Layout map

| Area | Path | Owns |
|---|---|---|
| Shell | `src/app/` | Router, `AppShell`, `Sidebar`, theme provider, window-level chrome |
| Features | `src/features/<domain>/` | Product pages, domain components, feature hooks |
| Shared | `src/shared/` | Cross-feature `ui/`, `composites/`, IPC hooks, lib, types |
| Design system | `packages/design-system/` | `--ds-*` tokens, recipes, barrel primitives |

**Shell vs features:** extend shared shell chrome; do not fork nav, theme, or
window framing inside a feature. Domain UI stays in its feature folder.

## Conventions

| Topic | Rule |
|---|---|
| Path aliases | `@/*`, `@shared/*`, `@features/*`, `@app/*` only — no deep `../../` imports. |
| Theming | Colors/type from **`--ds-*`**. Themes: **light \| dark** via `html[data-theme]`. Prefer mapped utilities (`text-fg`, `bg-glass`, `rounded-panel`, `text-ui*`). |
| Brand vs wash | `bg-brand` / `text-brand` = primary CTA (`--ds-accent`). `bg-control-hover` and shadcn-compat `accent` = **neutral wash**. **Inputs never use brand blue** — control wash + `border-separator`; focus stays neutral gray. |
| Recipes | Prefer DS recipes (`island`, `liquid-glass`, `glass`, `capsule`) for new chrome. Do not approximate with bare `backdrop-blur`. Legacy `glass-*` classes are migration debt — do not add new ones. |
| Typography | Use the DS type scale utilities (`text-ui` … `text-display`). No ad-hoc `text-[Npx]` in new code. |
| Logic | Non-trivial UI rules live in framework-free modules with colocated unit tests. |
| Stack | Fixed: React 19, Vite, Tauri 2, Tailwind v4, `@klyntbot/design-system`, lucide, custom IPC hooks, Biome, Vitest. **No new runtime UI deps without owner approval.** |

## Components

- Import UI from the **`@klyntbot/design-system` barrel** or **`@shared/ui` /
  `@shared/composites` re-exports**. No deep imports of
  `@klyntbot/design-system/...` (CSS entry `styles/index.css` is the only
  exception).
- Reusable, theme-agnostic, domain-free controls belong in the design-system (or
  get promoted there) with a barrel export — not copied across features.
- Domain types stay out of `packages/design-system`.
- Prefer existing shared primitives (Button, Input, Toggle, Dialog, Badge,
  Skeleton, Tooltip, …) before inventing a parallel `components/ui/` island
  under a feature.
- Feature-local one-offs are fine; promote when a second feature needs them.

## Brand / wash quick examples

```tsx
// CTA
<button className="bg-brand text-white …">Save</button>

// Input / wash (never brand)
<input className="bg-control-hover border-separator …" />

// Glass chrome
<aside className="island …" />
```

## Done gates

Before calling UI work done:

1. `bun run typecheck` (from `desktop-ui/`)
2. `bun run lint` (Biome)
3. Unit tests for pure logic touched by the change
4. **Light + dark** visual pass of touched surfaces
5. `bun run check:tokens` when touching colors, chrome, or shared styles

Perf-sensitive shell/dependency changes also follow
[`frontend-performance.md`](./frontend-performance.md).

## Not a design-system catalog

Conventions only. Component inventory and token scales live in the package +
[`design-tokens.md`](./design-tokens.md). This document is not a Storybook
substitute or a generated API catalog.

## Related

- [`design-tokens.md`](./design-tokens.md) — token contract
- [`frontend-performance.md`](./frontend-performance.md) — budgets / splitting
- [`jdbot-fe-upgrade-gaps.md`](./jdbot-fe-upgrade-gaps.md) — P0–P2 wave plan
- [`desktop-ui/AGENTS.md`](../../desktop-ui/AGENTS.md) — pointer for agents
