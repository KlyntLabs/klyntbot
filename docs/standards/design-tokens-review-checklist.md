# Design-system migration review checklist

Status: Review aid  
Contract: [`design-tokens.md`](./design-tokens.md)  
Package: `@klyntbot/design-system`

Use this when reviewing PRs that migrate `desktop-ui` (or related CSS) onto the design system. Every item is a verify/fail gate — not aspirational style notes.

---

## 1. Token contract violations

- [ ] **No raw color literals outside tokens** — `#hex`, `rgb()`, `rgba()` only in `packages/design-system/src/tokens/`. Domain CSS (`prose.css`, `editor.css`) may stay plain CSS but must use `var(--ds-*)` / mapped utilities, not hardcoded colors.
- [ ] **Canonical namespace** — product code prefers mapped utilities (`text-fg`, `bg-glass`, `rounded-panel`, …); raw `var(--ds-*)` is second choice. Do not invent new CSS vars in feature CSS.
- [ ] **Barrel imports only** — `from "@klyntbot/design-system"` only. Fail deep imports (`@klyntbot/design-system/...`).
- [ ] **No new entries in `compat.css`** — that file is temporary; Wave 4 deletes it. New aliases belong as proper `--ds-*` + `@theme` maps.
- [ ] **No ad-hoc `z-[N]`** — use mapped bands (`z-raised` … `z-toast`).
- [ ] **No domain types / product enums inside the design-system package.**

## 2. Accent vs brand (easy to get wrong)

| Utility / var | Means | Use for |
|---|---|---|
| `bg-brand` / `text-brand` / `--color-brand` → `--ds-accent` | Primary **blue** | CTAs, primary buttons |
| `bg-accent` / `text-accent` / `--color-accent` → `--ds-control-hover` | **Neutral wash** | Muted fills, hover chrome — **not** brand |
| `bg-control-hover` + `border-separator` | Control surface | Text inputs, form fields |

- [ ] **Inputs are not brand blue** — text fields, selects, textareas use wash/control surfaces; focus stays **neutral gray** (not `ring`/`brand` blue fill).
- [ ] **`bg-accent` is never treated as “the blue accent”** — it is shadcn-compat wash. Brand blue CTAs use `bg-brand`.
- [ ] Reviewers: watch for `bg-primary` / `var(--primary)` / `var(--ds-accent)` on form chrome — those paint brand blue.

> Naming trap: token `--ds-accent` **is** brand blue; utility `accent` / `--color-accent` is **not**. See skim notes below.

## 3. Light + dark theme

- [ ] Toggle `html[data-theme="light"|"dark"]` (or unset = light via `:root`) — both themes must render correctly.
- [ ] No reliance on Tailwind’s default `.dark` class alone — `dark:` is redefined against `data-theme` in `styles/variants.css`.
- [ ] Surfaces that hardcode light-only or dark-only colors without token overrides fail.
- [ ] Retro / extra themes are retired — product is **light | dark** only.

## 4. Glass recipes vs bare blur

- [ ] Floating / HUD / panel chrome uses DS recipes: `glass`, `glass-strong`, `liquid-glass`, `island`, `capsule`, `glass-blur`.
- [ ] Fail bare `backdrop-blur-*` or `backdrop-filter: blur(...)` without saturate — recipes use `var(--ds-blur*)` (blur **+** saturate).
- [ ] In-window panes that should not double-blur prefer `island` (no second backdrop-filter).

## 5. `cn()` type-scale registration

- [ ] Class composition uses `cn` from `@klyntbot/design-system` (not a local `twMerge` / `clsx`-only helper) when mixing `text-ui*` / `text-title*` with `text-fg*`.
- [ ] If a new type step is added: register in `tokens/typography.css`, map in `styles/theme.css` `@theme inline`, **and** add the suffix to `dsFontSizeTokens` in `packages/design-system/src/lib/cn.ts`. Missing registration → `tailwind-merge` treats `text-ui` as a color and silently drops it next to `text-fg`.
- [ ] No hardcoded `font-size: Npx` in chrome UI — use `text-ui-xs` … `text-display` (domain `prose.css` / `editor.css` may keep relative sizes but should migrate toward tokens over time).

## 6. `compat.css` retirement criteria

Ready to delete `packages/design-system/src/styles/compat.css` (and its `@import` in `styles/index.css`) only when **all** are true:

- [ ] No product CSS/TS references bare legacy vars (`var(--background)`, `var(--primary)`, `var(--accent)`, `var(--surface-glass*)`, `var(--brand)`, …).
- [ ] No reliance on `@theme` **temporary migration aliases** in `theme.css` (`--color-background`, `--color-primary`, `--color-muted`, `--color-sidebar-*`, surface ladder, etc.) — prefer canonical `--color-bg` / `--color-brand` / `--color-fg` maps; plan a follow-up to strip those aliases once consumers are clean.
- [ ] Grep gate is clean across `desktop-ui/` (and any other consumers).
- [ ] Visual pass on high-risk surfaces (below) in light + dark after removal.

Until then: **do not add** to `compat.css`; migrate callers forward instead.

## 7. High-risk surfaces to eyeball

Toggle light/dark and check contrast, focus rings, glass, and input wash (not brand fill):

| Surface | What to watch |
|---|---|
| **Settings forms** | Inputs/`bg-accent` wash vs brand; toggles; section chrome; focus not blue-washed |
| **Chat** | Bubbles, composer, markdown/`prose.css`, streaming chrome, selection |
| **Notes editor** | `editor.css` token usage, headings, code blocks, selection, any leftover hex fallbacks |
| **Launcher / tray HUD** | `liquid-glass` / floating recipes, transparency over desktop, drag chrome |
| **Tasks** | List rows, controls, status colors via tokens (not one-off hex) |

Also spot-check: tray, distraction overlay, voice orb if touched by the PR.

---

## Skim notes: `theme.css` vs `compat.css` (brand / accent)

Checked: `packages/design-system/src/styles/theme.css`, `compat.css`, and `tokens/color.css`.

### No value conflict on the dual accent story

Both layers intentionally encode the same semantics:

| Name | Resolves to | Role |
|---|---|---|
| `--color-brand` / `--brand` / `--color-primary` / `--primary` | `var(--ds-accent)` (blue `#0a7cff`) | CTA / brand |
| `--color-accent` / `--accent` / `--color-muted` / `--muted` | `var(--ds-control-hover)` (neutral wash) | Wash / muted fill |

So **`--color-brand` and `--color-accent` are not duplicates of each other** — they map to different `--ds-*` tokens. `compat.css` mirrors that with unprefixed shadcn/Tahoe names.

### Duplicate surfaces (migration overlap, not opposing values)

- **Prefixed vs bare:** `theme.css` exposes Tailwind `--color-*`; `compat.css` re-aliases the same concepts as bare `--primary`, `--accent`, `--brand`, `--surface-glass*`, etc. Same targets, two APIs — expected until Wave 4.
- **`--ds-accent` naming trap:** the **token** `--ds-accent` is brand blue; the **utility** `accent` / `--color-accent` is wash. Reviewers and authors confuse these constantly.
- **Input focus tension:** contract says input focus stays neutral gray; `@theme` still maps `--color-ring` → `--ds-accent` (brand). Flag `ring-*` / `focus-visible:ring-*` on inputs if they reintroduce brand-blue focus chrome.
- **`compat.css` only defines `:root` aliases** — it does not re-specify dark values; dark works because aliases point at `--ds-*` that flip under `html[data-theme="dark"]`. Do not “fix” dark by copying hex into `compat.css`.

### Related contract pointers

- Token SoT: `packages/design-system/src/tokens/`
- Theme map: `packages/design-system/src/styles/theme.css`
- Compat (temporary): `packages/design-system/src/styles/compat.css`
- Glass recipes: `packages/design-system/src/recipes/recipes.css`
- `cn` scale list: `packages/design-system/src/lib/cn.ts`
