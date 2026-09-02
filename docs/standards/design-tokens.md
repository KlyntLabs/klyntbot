# Design tokens & UI stack (KlyntBot)

Status: Approved  
Date: 2026-09-02

## Purpose

Visual contract for `desktop-ui`: token vocabulary, scales, forbidden patterns, and how product code consumes `@klyntbot/design-system`. Adapted from the klynt platform design system without Astryx or OS-shell chrome.

## Stack

| Layer | Choice |
|---|---|
| UI | React 19 + Vite + Tauri 2 |
| Styling | Tailwind v4 utilities mapped from `--ds-*` via `@theme inline` |
| Components | `@klyntbot/design-system` barrel + Radix primitives in-app |
| Icons | `lucide-react` |

## Token source of truth

- Raw values live only in `packages/design-system/src/tokens/`.
- Canonical namespace: `--ds-*`.
- Map to utilities in `packages/design-system/src/styles/theme.css`.
- Prefer mapped utility (`text-fg`, `bg-glass`, `rounded-panel`) then `var(--ds-*)`.
- Domain palettes (timeline, origin, chart) live in `tokens/domain.css`.

## Themes

- Light: `:root`
- Dark: `html[data-theme="dark"]`
- Tailwind `dark:` is redefined against `data-theme` in `styles/variants.css`
- Product themes: **light | dark** only (Retro retired)

## Accent vs brand (important)

| Utility | Role |
|---|---|
| `bg-brand` / `text-brand` | Primary blue CTA (`--ds-accent`) |
| `bg-accent` / `text-accent` | **Neutral wash** (shadcn-compat → `--ds-control-hover`) — form fields, muted fills |

Do **not** paint text inputs with brand blue. Inputs use `bg-control-hover` + `border-separator`; focus stays neutral gray.

## Scales (summary)

- Type: `ui-xs` 11 → `ui-sm` 12 → `ui` 13 → `body` 14 → `title-sm` 17 → `title` 20 → `title-lg` 22 → `display-sm` 26 → `display` 52
- Space: 4px base (`--ds-space-*`)
- Radius by role: `control`, `menu`, `panel`, `card`, …
- Motion: `--ds-duration-*` + `--ds-ease-*`
- Z bands: `raised`, `sticky`, `menu`, `overlay`, `modal`, `toast`

## Recipes

Use `@utility` recipes for glass — include blur **and** saturate:

- `glass`, `glass-strong`, `liquid-glass`, `island`, `capsule`, `glass-blur`

## Forbidden

- Raw hex / `rgb()` / `rgba()` outside `packages/design-system/src/tokens/`
- Deep imports of `@klyntbot/design-system/...` (barrel only)
- Domain types inside the design-system package
- Approximating glass with bare `backdrop-filter: blur(...)` (drops saturate)
- Ad-hoc `z-[N]` instead of mapped bands
- Web fonts without an explicit decision

## `cn()`

`cn` from the package registers DS font-size tokens with `extendTailwindMerge`. Always use it when composing `text-ui` / `text-fg` together.

## Migration aliases

Legacy `styles/compat.css` was removed after callers moved to `--ds-*` / mapped utilities. Do not reintroduce bare shadcn/Tahoe var aliases — extend `--ds-*` + `@theme` instead.
