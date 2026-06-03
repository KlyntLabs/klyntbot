# Klynt Design System — Architecture Analysis & Upgrade Path

> **Date**: 2026-06-03
> **Status**: Analysis complete, ready for implementation
> **Scope**: Color tokens, theming architecture, multi-theme support

---

## Current State Analysis

### Token Inventory

| Category | Count | Examples |
|----------|-------|----------|
| Text colors | 10 | `text-primary` → `text-dim` (10-step greyscale) |
| Surface colors | 21 | `surface-sidebar` → `surface-popover` (location-based) |
| Border colors | 8 | `border-subtle` → `border-review` |
| Status colors | 4 | `status-success`, `warning`, `error`, `unknown` |
| Accent colors | 3 | `text-accent`, `text-accent-cyan`, `shadow-accent` |
| CodeMirror chips | 33 | 11 colors × 3 variants (bg/fg/border) |
| **Total color tokens** | **~80** | |

### Theme Files

```
src/styles/
├── themes.css           (4 imports)
├── themes.dark.css      (~110 lines, RGBA hardcoded)
├── themes.dim.css       (~72 lines, RGBA hardcoded)
├── themes.light.css     (~92 lines, RGBA hardcoded)
└── themes.system.css    (~86 lines, RGBA hardcoded, media-query based)
```

**Problem**: To add a new theme (e.g., "high-contrast", "ocean", "sepia"), you must create a new ~90-line file copying the same structure and manually editing every RGBA value.

### What's Wrong With RGBA?

```css
/* Current: RGBA — impossible to reason about */
--surface-card: rgba(255, 255, 255, 0.04);
--surface-card-strong: rgba(255, 255, 255, 0.12);
--surface-card-muted: rgba(255, 255, 255, 0.06);

/* If you want to darken "card-strong" by 10%, what RGBA do you write? */
/* You have to eyeball it. Every. Single. Time. */
```

RGBA has no relationship between `card`, `card-strong`, and `card-muted`. They're independent guesses.

---

## The Upgrade: OKLCH + Semantic Tokens + Derived Colors

### Why OKLCH?

| Color Space | Lighten by 20% | Result |
|-------------|---------------|--------|
| **RGB** | `#3b82f6` → ??? | `#7aa7f8` (guesswork) |
| **HSL** | `hsl(217 91% 60%)` → `hsl(217 91% 80%)` | Lighter but hue shifts perceptually |
| **OKLCH** | `oklch(60% 0.2 255)` → `oklch(80% 0.2 255)` | **Exactly 20% lighter, same hue/chroma** |

OKLCH = **O**klab **L**ightness **C**hroma **H**ue. It's the only color space where changing `L` (lightness) doesn't shift the perceived hue.

> Browser support: 96%+ (Chrome 111+, Safari 16.4+, Firefox 113+)

### The 3-Layer Token Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  LAYER 1: PRIMITIVES (OKLCH values)                          │
│  --color-base-hue: 255                                       │
│  --color-base-chroma: 0.05                                   │
│  --color-accent-hue: 210                                     │
│  --color-accent-chroma: 0.18                                 │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  LAYER 2: SEMANTIC (purpose-based, derived via color-mix)   │
│  --color-bg: oklch(15% 0.01 255)                             │
│  --color-bg-elevated: color-mix(in oklch, --color-bg, white 8%)│
│  --color-fg: oklch(95% 0.01 255)                             │
│  --color-fg-muted: color-mix(in oklch, --color-fg, --color-bg 40%)│
│  --color-border: color-mix(in oklch, --color-fg, --color-bg 92%) │
│  --color-accent: oklch(70% 0.18 210)                         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  LAYER 3: COMPONENT (optional, specific overrides)           │
│  --composer-bg: var(--color-bg-elevated)                     │
│  --sidebar-bg: var(--color-bg)                               │
│  --message-bubble-user: color-mix(in oklch, --color-accent 50%, transparent)│
└─────────────────────────────────────────────────────────────┘
```

**Key insight**: Layer 2 tokens are **derived from Layer 1** using `color-mix()`. Change the base hue in Layer 1, and the entire app shifts color automatically.

---

## Proposed New Architecture

### File Structure

```
src/styles/
├── app.css                          ← Main entry (imports everything)
├── tokens/
│   ├── primitives.css               ← OKLCH base values (hue, chroma scales)
│   ├── semantic.css                 ← Derived tokens (bg, fg, border, accent)
│   ├── colors-extended.css          ← Status, CM chips, tool families
│   ├── typography.css               ← Font families, size scale
│   ├── spacing.css                  ← Spacing scale
│   ├── motion.css                   ← Durations, easings
│   └── radius.css                   ← Border radius scale
├── themes/
│   ├── _base.css                    ← Shared theme structure
│   ├── dark.css                     ← Dark overrides (only primitives!)
│   ├── dim.css                      ← Dim overrides
│   ├── light.css                    ← Light overrides
│   └── system.css                   ← prefers-color-scheme wrapper
└── components/                      ← Feature-specific @utility (optional)
    └── (legacy BEM → @utility conversions)
```

### The Magic: Themes Only Override Primitives

```css
/* themes/dark.css — ONLY 12 lines! */
[data-theme="dark"] {
  --primitive-bg-l: 15%;
  --primitive-fg-l: 95%;
  --primitive-accent-hue: 210;
  --primitive-accent-chroma: 0.18;
  --primitive-muted-chroma: 0.01;
}

/* themes/light.css — ONLY 12 lines! */
[data-theme="light"] {
  --primitive-bg-l: 98%;
  --primitive-fg-l: 15%;
  --primitive-accent-hue: 210;
  --primitive-accent-chroma: 0.16;
  --primitive-muted-chroma: 0.01;
}
```

All the semantic tokens (`--color-bg`, `--color-fg`, `--color-border`, etc.) are derived automatically via `color-mix()` in `tokens/semantic.css`. **You never touch them when adding a theme.**

### Simplified Semantic Token Set

| Old Token (×80) | New Token (×12) | Purpose |
|-----------------|-----------------|---------|
| `text-primary`, `text-strong`, `text-emphasis`, `text-stronger`, `text-quiet`, `text-muted`, `text-subtle`, `text-faint`, `text-fainter`, `text-dim` | `--color-fg`, `--color-fg-muted`, `--color-fg-faint` | Text hierarchy |
| `surface-sidebar`, `surface-topbar`, `surface-messages`, `surface-composer`, `surface-card`, `surface-card-strong`, `surface-card-muted`, `surface-item`, `surface-control`, `surface-hover`, `surface-active`, `surface-popover`, `surface-command`, `surface-approval`, `surface-debug`, `surface-diff-card`, `surface-bubble`, `surface-bubble-user`, `surface-context-core`, `surface-right-panel` | `--color-bg`, `--color-bg-elevated`, `--color-bg-sunken`, `--color-bg-hover`, `--color-bg-active`, `--color-bg-overlay` | Surface hierarchy |
| `border-subtle`, `border-muted`, `border-strong`, `border-stronger`, `border-quiet`, `border-accent`, `border-accent-soft`, `border-review` | `--color-border`, `--color-border-strong`, `--color-border-accent` | Border hierarchy |

**From 80 tokens → ~18 semantic tokens.**

### Component Tokens (Layer 3)

Only define component tokens when a component needs something specific:

```css
:root {
  /* Default: alias to semantic */
  --composer-bg: var(--color-bg-elevated);
  --sidebar-bg: var(--color-bg);
  --topbar-bg: var(--color-bg-elevated);
  --message-bubble-bg: var(--color-bg-sunken);
  --message-bubble-user-bg: color-mix(in oklch, var(--color-accent) 45%, transparent);
  --review-active-bg: color-mix(in oklch, oklch(70% 0.2 340) 18%, transparent);
}
```

### Example: Adding a "High Contrast" Theme

```css
/* themes/high-contrast.css — 6 lines! */
[data-theme="high-contrast"] {
  --primitive-bg-l: 5%;
  --primitive-fg-l: 100%;
  --primitive-accent-hue: 210;
  --primitive-accent-chroma: 0.25;  /* More saturated */
  --primitive-muted-chroma: 0.02;
  --primitive-contrast-boost: 1.5;  /* Custom primitive for high contrast */
}
```

Done. Every derived color updates automatically.

### Example: Adding a "Brand" Theme (Ocean)

```css
/* themes/ocean.css — 4 lines! */
[data-theme="ocean"] {
  --primitive-accent-hue: 190;      /* Teal instead of blue */
  --primitive-accent-chroma: 0.15;
}
```

The entire app shifts to teal accents. Backgrounds, borders, highlights — everything derived from the accent updates.

---

## Implementation Plan

### Phase 1: Create New Token System (Non-Breaking)

1. Create `src/styles/tokens/` directory
2. Write `primitives.css` with OKLCH base values
3. Write `semantic.css` with derived tokens
4. Write `colors-extended.css` for status/chips
5. Create new theme files in `src/styles/themes/` (dark.css, light.css, dim.css)
6. Update `app.css` to import new tokens alongside existing ones

**At this point**: Both old and new tokens coexist. Nothing breaks.

### Phase 2: Migrate Components (Gradual)

1. Update atomic components to use new semantic tokens:
   - `bg-surface-card` → `bg-bg-elevated`
   - `text-text-muted` → `text-fg-muted`
   - `border-border-subtle` → `border-border`
2. Update `app.css` `@theme inline` to bridge new tokens

### Phase 3: Remove Legacy Tokens

1. Delete old theme files (`themes.dark.css`, `themes.light.css`, etc.)
2. Remove old CSS custom properties from components
3. Clean up unused `@theme inline` entries

---

## Code Samples

### `tokens/primitives.css`

```css
:root {
  /* ── Hue wheel position (0-360) ── */
  --primitive-neutral-hue: 255;
  --primitive-accent-hue: 210;
  --primitive-success-hue: 155;
  --primitive-warning-hue: 75;
  --primitive-error-hue: 25;

  /* ── Chroma (colorfulness, 0-0.4) ── */
  --primitive-muted-chroma: 0.01;
  --primitive-accent-chroma: 0.18;
  --primitive-status-chroma: 0.15;

  /* ── Lightness (0% = black, 100% = white) ── */
  /* These are overridden by theme files */
  --primitive-bg-l: 15%;
  --primitive-fg-l: 95%;
  --primitive-elevated-l-offset: 6%;
  --primitive-sunken-l-offset: -3%;
}
```

### `tokens/semantic.css`

```css
:root {
  /* ── Backgrounds ── */
  --color-bg: oklch(var(--primitive-bg-l) var(--primitive-muted-chroma) var(--primitive-neutral-hue));
  --color-bg-elevated: oklch(calc(var(--primitive-bg-l) + var(--primitive-elevated-l-offset)) var(--primitive-muted-chroma) var(--primitive-neutral-hue));
  --color-bg-sunken: oklch(calc(var(--primitive-bg-l) - var(--primitive-sunken-l-offset)) var(--primitive-muted-chroma) var(--primitive-neutral-hue));
  --color-bg-hover: color-mix(in oklch, var(--color-fg) 6%, var(--color-bg));
  --color-bg-active: color-mix(in oklch, var(--color-accent) 18%, var(--color-bg));
  --color-bg-overlay: oklch(calc(var(--primitive-bg-l) - 2%) var(--primitive-muted-chroma) var(--primitive-neutral-hue) / 0.95);

  /* ── Foregrounds ── */
  --color-fg: oklch(var(--primitive-fg-l) var(--primitive-muted-chroma) var(--primitive-neutral-hue));
  --color-fg-muted: color-mix(in oklch, var(--color-fg) 60%, var(--color-bg));
  --color-fg-faint: color-mix(in oklch, var(--color-fg) 35%, var(--color-bg));
  --color-fg-accent: oklch(75% var(--primitive-accent-chroma) var(--primitive-accent-hue));

  /* ── Borders ── */
  --color-border: color-mix(in oklch, var(--color-fg) 8%, var(--color-bg));
  --color-border-strong: color-mix(in oklch, var(--color-fg) 18%, var(--color-bg));
  --color-border-accent: color-mix(in oklch, var(--color-accent) 50%, transparent);

  /* ── Accent ── */
  --color-accent: oklch(70% var(--primitive-accent-chroma) var(--primitive-accent-hue));
  --color-accent-strong: oklch(80% calc(var(--primitive-accent-chroma) * 1.2) var(--primitive-accent-hue));

  /* ── Status ── */
  --color-success: oklch(75% var(--primitive-status-chroma) var(--primitive-success-hue));
  --color-warning: oklch(78% var(--primitive-status-chroma) var(--primitive-warning-hue));
  --color-error: oklch(70% var(--primitive-status-chroma) var(--primitive-error-hue));
}
```

### `themes/dark.css`

```css
[data-theme="dark"] {
  color-scheme: dark;
  --primitive-bg-l: 15%;
  --primitive-fg-l: 95%;
  --primitive-accent-hue: 210;
  --primitive-accent-chroma: 0.18;
}
```

### `themes/light.css`

```css
[data-theme="light"] {
  color-scheme: light;
  --primitive-bg-l: 98%;
  --primitive-fg-l: 12%;
  --primitive-accent-hue: 210;
  --primitive-accent-chroma: 0.16;
}
```

---

## Comparison: Old vs New

| Aspect | Current (RGBA) | Proposed (OKLCH) |
|--------|---------------|------------------|
| Theme file size | ~110 lines | ~5 lines |
| Color tokens | ~80 | ~18 |
| Add new theme | Copy 110 lines, edit 80 values | Edit 4-6 primitives |
| Change accent color | Find/replace across 4 files | Change 1 `--primitive-accent-hue` |
| Generate tints/shades | Eyeball RGBA | `color-mix()` with OKLCH |
| Dark mode contrast | Manual adjustment | Automatic via lightness inversion |
| Runtime theming | Not feasible | `document.documentElement.style.setProperty('--primitive-accent-hue', '120')` |

---

## Multi-Theme Scenarios Enabled

| Scenario | How It Works |
|----------|-------------|
| **User picks "Ocean" theme** | `--primitive-accent-hue: 190` — everything shifts teal |
| **User picks "High Contrast"** | Increase chroma, reduce mixing ratios |
| **User picks "Sepia"** | `--primitive-neutral-hue: 75` — warm paper tones |
| **Seasonal event (Halloween)** | `--primitive-accent-hue: 45` — orange accents |
| **Brand white-label** | Change accent hue + chroma per client |
| **Accessibility: colorblind-safe** | Reduce chroma, increase lightness contrast |
| **Auto dark/light** | `prefers-color-scheme` toggles bg-l / fg-l |
| **AMOLED black** | `--primitive-bg-l: 0%` — pure black background |

---

## Appendix: Current Token Crosswalk

| Current Token | Maps To New |
|--------------|-------------|
| `--text-primary` | `--color-fg` |
| `--text-strong` | `--color-fg` |
| `--text-muted` | `--color-fg-muted` |
| `--text-faint` | `--color-fg-faint` |
| `--surface-sidebar` | `--color-bg` |
| `--surface-topbar` | `--color-bg-elevated` |
| `--surface-messages` | `--color-bg` |
| `--surface-card` | `--color-bg-elevated` |
| `--surface-card-strong` | `color-mix(in oklch, --color-bg-elevated, --color-fg 10%)` |
| `--surface-control` | `--color-bg-sunken` |
| `--surface-hover` | `--color-bg-hover` |
| `--surface-active` | `--color-bg-active` |
| `--border-subtle` | `--color-border` |
| `--border-strong` | `--color-border-strong` |
| `--border-accent` | `--color-border-accent` |
| `--text-accent-cyan` | `--color-fg-accent` |
| `--status-success` | `--color-success` |
| `--status-error` | `--color-error` |
