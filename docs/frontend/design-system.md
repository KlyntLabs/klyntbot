# Design System

## 1. Overview

The design system uses Tailwind CSS v4 with CSS-first theming. There is no `tailwind.config.js`. All design tokens are defined as CSS custom properties in `:root`, then bridged to Tailwind utilities via `@theme inline` blocks. The visual language is called "Tahoe" -- a dark glassmorphism aesthetic inspired by macOS vibrancy materials.

Themes are applied via a `data-theme` attribute on the root element (e.g., `data-theme="dark"`, `data-theme="retro"`). A `ThemeProvider` component manages the active theme.

### File Organization

```
src/styles/
  index.css             # Entry point: imports all style files
  tailwind.css          # Tailwind v4 base (@import "tailwindcss")
  theme.css             # Core token definitions + glass utilities + animations
  prose.css             # Markdown/prose typography
  editor.css            # TipTap editor styles
  fonts.css             # @font-face declarations

src/shared/styles/
  glass.css             # Theme-agnostic glass material utility classes
  themes/
    _base.css           # Structural tokens (spacing, radius, typography, animation)
    dark.css            # Dark theme color tokens + @theme inline registration
    retro.css           # Retro 90s theme: opaque surfaces, chunky borders, hard shadows
```

## 2. Color Token System

All colors are defined as CSS custom properties on `:root` (dark theme) and overridden per-theme via `[data-theme="..."]` selectors.

### Background

| Variable | Dark Value | Purpose |
|---|---|---|
| `--background` | `#000000` | Page background |

### Surface Staircase

Five elevation tiers using increasing white alpha values, creating depth without drop shadows:

| Variable | Dark Value | Purpose |
|---|---|---|
| `--surface-lowest` | `rgba(255,255,255, 0.025)` | Deepest recessed areas |
| `--surface-low` | `rgba(255,255,255, 0.04)` | Below-baseline surfaces |
| `--surface-base` | `rgba(255,255,255, 0.06)` | Default content surface |
| `--surface-raised` | `rgba(255,255,255, 0.09)` | Elevated cards, hover states |
| `--surface-highest` | `rgba(255,255,255, 0.13)` | Top-level floating elements |

### Text Hierarchy

| Variable | Dark Value | Purpose |
|---|---|---|
| `--text-primary` | `#f0f2f5` | Headings, primary content |
| `--text-secondary` | `#c8cdd4` | Body text, descriptions |
| `--text-muted` | `#7d8590` | Labels, placeholders |
| `--text-dim` | `#5a616b` | Disabled, tertiary text |

### Brand

| Variable | Dark Value | Purpose |
|---|---|---|
| `--brand` | `#f97316` | Primary accent (orange) |
| `--brand-hover` | `#fb923c` | Brand hover state |
| `--brand-glow` | `rgba(249,115,22, 0.25)` | Brand glow/halo effect |

### Semantic Colors

| Variable | Dark Value | Purpose |
|---|---|---|
| `--success` | `#34d399` | Success states, positive |
| `--destructive` | `#f43f5e` | Error, delete, danger |
| `--info` | `#60a5fa` | Informational highlights |
| `--warning` | `#fbbf24` | Warning states |
| `--purple` | `#a78bfa` | Accent / AI-related |

### Origin Badges

Used to color-code the source of content:

| Variable | Dark Value | Purpose |
|---|---|---|
| `--origin-system` | `#60a5fa` | System-generated |
| `--origin-ai` | `#a78bfa` | AI-generated |
| `--origin-user` | `#34d399` | User-created |
| `--origin-plugin` | `#fbbf24` | Plugin-generated |

### Border

| Variable | Dark Value | Purpose |
|---|---|---|
| `--border` | `rgba(255,255,255, 0.08)` | Default borders |
| `--border-subtle` | `rgba(255,255,255, 0.04)` | Subtle separators |

### Timeline / Dashboard

Specialized colors for the productivity timeline and dashboard visualizations:

| Variable | Purpose |
|---|---|
| `--timeline-app-productive` | Productive app usage |
| `--timeline-app-distracting` | Distracting app usage |
| `--timeline-app-neutral` | Neutral app usage |
| `--timeline-focus` | Focus session |
| `--timeline-focus-high` / `--timeline-focus-low` | Focus intensity gradient |
| `--timeline-task` | Task activity |
| `--timeline-note` | Note activity |
| `--timeline-finance` | Finance activity |
| `--timeline-todo` | Todo activity |
| `--timeline-finance-expense` / `--timeline-finance-income` | Expense vs income |
| `--timeline-system` | System events |
| `--timeline-calendar` | Calendar events |
| `--timeline-unfocused` | Unfocused time blocks |
| `--timeline-dot-note` / `--timeline-dot-task-done` / `--timeline-dot-finance` | Timeline dot markers |

These use `oklch()` color space for perceptually uniform brightness across the palette.

### Overlay

| Variable | Dark Value | Purpose |
|---|---|---|
| `--overlay` | `rgba(0,0,0, 0.25)` | Modal backdrop |
| `--overlay-heavy` | `rgba(0,0,0, 0.45)` | Heavy overlay (alerts) |

### Glass Material Variables

| Variable | Dark Value | Purpose |
|---|---|---|
| `--surface-floating` | `rgba(10,10,10, 0.92)` | Floating panel background |
| `--surface-glass` | `rgba(255,255,255, 0.08)` | Primary glass material |
| `--surface-glass-sidebar` | `rgba(255,255,255, 0.06)` | Sidebar glass (more translucent) |
| `--surface-glass-subtle` | `rgba(255,255,255, 0.06)` | Input/button glass |
| `--surface-glass-subtle-hover` | `rgba(255,255,255, 0.1)` | Input/button glass hover |
| `--surface-glass-elevated` | `rgba(255,255,255, 0.14)` | Active/pressed glass |
| `--glass-border` | `rgba(255,255,255, 0.12)` | Glass element border |
| `--glass-border-strong` | `rgba(255,255,255, 0.2)` | Emphasized glass border |
| `--glass-shadow` | `none` | Glass shadow (dark has none) |
| `--glass-padding` | `6px` | Inner padding for glass panels |
| `--glass-radius-inner` | `calc(radius-xl - 6px)` | Inner content border radius |
| `--glass-tint-brand` | `rgba(249,115,22, 0.08)` | Brand-tinted glass |
| `--glass-tint-info` | `rgba(96,165,250, 0.06)` | Info-tinted glass |
| `--glass-tint-success` | `rgba(52,211,153, 0.06)` | Success-tinted glass |
| `--glass-tint-destructive` | `rgba(244,63,94, 0.06)` | Destructive-tinted glass |

## 3. @theme inline Bridge

Tailwind v4 uses `@theme inline` blocks to register CSS variables as Tailwind utility classes. This is the mechanism that lets you write `bg-surface-base` or `text-muted` in JSX.

The pattern:

```css
/* 1. Define the raw variable in :root */
:root {
  --surface-base: rgba(255, 255, 255, 0.06);
}

/* 2. Register it as a Tailwind color in @theme inline */
@theme inline {
  --color-surface-base: var(--surface-base);
}

/* 3. Use in components */
/* <div className="bg-surface-base"> */
```

The `--color-*` prefix is Tailwind v4's convention for color utilities. Registering `--color-surface-base` enables `bg-surface-base`, `text-surface-base`, `border-surface-base`, etc.

### Registered Utility Colors

All of the following work as Tailwind color utilities (e.g., `bg-*`, `text-*`, `border-*`):

- **Surface:** `surface-lowest`, `surface-low`, `surface-base`, `surface-raised`, `surface-highest`, `surface-floating`, `surface-glass`
- **Text:** `primary`, `secondary`, `muted`, `dim`
- **Brand:** `brand`, `brand-hover`
- **Semantic:** `success`, `destructive`, `info`, `warning`, `purple`
- **Origin:** `origin-system`, `origin-ai`, `origin-user`, `origin-plugin`
- **Border:** `border` (default), `border-subtle`
- **Overlay:** `overlay`, `overlay-heavy`
- **Timeline:** All `timeline-*` variables listed above

### Registered Radius Tokens

```css
@theme inline {
  --radius-sm: calc(var(--radius) - 6px);   /* 6px  */
  --radius-md: calc(var(--radius) - 4px);   /* 8px  */
  --radius-lg: var(--radius);               /* 12px */
  --radius-xl: calc(var(--radius) + 4px);   /* 16px */
  --radius-2xl: calc(var(--radius) + 8px);  /* 20px */
  --radius-pill: 9999px;
}
```

These enable `rounded-sm`, `rounded-md`, `rounded-lg`, `rounded-xl`, `rounded-2xl`, and `rounded-pill`.

## 4. Glassmorphism Classes

All glass classes are defined in `src/shared/styles/glass.css` (and duplicated in `src/styles/theme.css` for the legacy path). They use `@apply` for backdrop-filter composition and CSS variables for theme-responsive colors.

### glass-panel

Primary glass material for dropdowns, popovers, and dialogs.

```
backdrop-blur: 80px
backdrop-saturate: 1.6
background: var(--surface-glass)
border: 1px solid var(--glass-border)
border-radius: var(--radius-xl)
box-shadow: var(--glass-shadow)
padding: var(--glass-padding)
```

### glass-sidebar

Sidebar navigation panel with heavier blur.

```
backdrop-blur: 100px
backdrop-saturate: 1.8
background: var(--surface-glass-sidebar)
border: 1px solid var(--glass-border)
border-radius: var(--radius-xl)
```

### glass-toolbar

Floating toolbar bars.

```
backdrop-blur: 60px
backdrop-saturate: 1.5
background: rgba(255, 255, 255, 0.07)
border: 1px solid var(--glass-border)
border-radius: var(--radius-xl)
```

### glass-input

Subtle glass for form inputs and text fields. Transitions background and border on `:focus-within`.

```
backdrop-blur: 40px
backdrop-saturate: 1.3
background: var(--surface-glass-subtle)         -> hover: var(--surface-glass-subtle-hover)
border: 1px solid rgba(255, 255, 255, 0.08)     -> focus: rgba(255, 255, 255, 0.14)
border-radius: var(--radius-xl)
```

### glass-button

Interactive glass buttons with hover and active states.

```
backdrop-blur: 30px
backdrop-saturate: 1.2
background: var(--surface-glass-subtle)
  -> hover: var(--surface-glass-subtle-hover)
  -> active: var(--surface-glass-elevated)
border: 1px solid rgba(255, 255, 255, 0.06)
  -> hover: rgba(255, 255, 255, 0.12)
  -> active: rgba(255, 255, 255, 0.18)
border-radius: var(--radius-lg)
```

### glass-floating

Popover windows rendered outside the main app (launcher, system tray). Uses dark frosted glass.

```
backdrop-blur: 80px
backdrop-saturate: 1.8
background: rgba(10, 10, 14, 0.82)
border: 1px solid rgba(255, 255, 255, 0.12)
border-radius: var(--radius-xl)
padding: var(--glass-padding)
```

When macOS native vibrancy is active (`[data-vibrancy]`), the CSS backdrop-filter is disabled and the background is lightened to let the native material show through.

### glass-dropdown

In-app floating menus and popovers. More translucent than `glass-floating` so the backdrop-blur is visible over app content.

```
backdrop-blur: 80px
backdrop-saturate: 1.8
background: rgba(10, 10, 14, 0.75)
border: 1px solid rgba(255, 255, 255, 0.12)
border-radius: var(--radius-xl)
box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5),
            inset 0 1px 0 rgba(255, 255, 255, 0.06)
padding: var(--glass-padding)
```

### glass-card

Content cards with subtle depth.

```
backdrop-blur: 50px
backdrop-saturate: 1.5
background: rgba(255, 255, 255, 0.04)
border: 1px solid rgba(255, 255, 255, 0.07)
border-radius: var(--radius-xl)
```

### glass-bubble / glass-bubble-user

Chat message bubbles. The user variant is slightly more opaque.

```
glass-bubble:
  backdrop-blur: 40px, backdrop-saturate: 1.4
  background: rgba(255, 255, 255, 0.07)
  border: 1px solid rgba(255, 255, 255, 0.09)
  border-radius: var(--radius-2xl)

glass-bubble-user:
  backdrop-blur: 40px, backdrop-saturate: 1.4
  background: rgba(255, 255, 255, 0.08)
  border: 1px solid rgba(255, 255, 255, 0.1)
  border-radius: var(--radius-2xl)
```

### glass-badge

Small pill-shaped badges.

```
backdrop-blur: 20px
background: rgba(255, 255, 255, 0.06)
border: 1px solid rgba(255, 255, 255, 0.08)
border-radius: var(--radius-pill)
```

### glass-divider

Horizontal separator with gradient fade at edges.

```
height: 1px
background: linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.08) 20%, rgba(255,255,255,0.08) 80%, transparent 100%)
```

### context-menu

macOS-native style context menu.

```
backdrop-blur: 80px
backdrop-saturate: 1.8
background: rgba(22, 22, 24, 0.88)
border: 1px solid rgba(255, 255, 255, 0.14)
border-radius: 10px
box-shadow: 0 8px 40px rgba(0,0,0,0.55), 0 2px 8px rgba(0,0,0,0.3), inset 0 0.5px 0 rgba(255,255,255,0.06)
```

### Additional Utility Classes

- **`note-card`** -- Interactive card with hover lift (`translateY(-1px)`) and active state with brand-colored left border.
- **`tag-pill`** / **`tag-pill-active`** -- Small tag labels with brand-tinted active state using `color-mix()`.
- **`version-dot`** / **`version-dot-active`** -- Timeline progress dots with brand glow on active.
- **`version-line`** -- Timeline connector with gradient opacity.
- **`tabular-nums`** -- Enables `font-variant-numeric: tabular-nums` for aligned numerical data.

### Performance: Resize Optimization

During panel resizing (`.resizing` parent class), `backdrop-blur` and `backdrop-saturate` are removed from `.glass-panel` and `.glass-input` to prevent GPU-intensive recomposition during drag.

## 5. Typography

### Font Stack

```
"SF Pro Display", "SF Pro Text", -apple-system, BlinkMacSystemFont, system-ui, "Helvetica Neue", sans-serif
```

The retro theme overrides this to `"Space Grotesk", "DM Sans", "Inter", system-ui, sans-serif`.

### Base Size

`--font-size: 15px` applied on `html`.

### Weight Scale

| Variable | Value | Usage |
|---|---|---|
| `--font-weight-light` | 300 | Inputs |
| `--font-weight-normal` | 400 | Body text |
| `--font-weight-medium` | 500 | Labels, buttons, h2-h4 |
| `--font-weight-semibold` | 600 | h1 headings |

### Heading Styles

- **h1:** `text-2xl`, semibold, `letter-spacing: -0.02em`, `text-wrap: balance`
- **h2:** `text-xl`, medium, `letter-spacing: -0.015em`
- **h3:** `text-lg`, medium, `letter-spacing: -0.01em`
- **h4:** `text-base`, medium

Font smoothing: `-webkit-font-smoothing: antialiased` and `-moz-osx-font-smoothing: grayscale` on `body`.

## 6. Spacing and Layout

### Spacing Scale

Defined in `_base.css`:

| Variable | Value |
|---|---|
| `--space-xs` | 4px |
| `--space-sm` | 8px |
| `--space-md` | 16px |
| `--space-lg` | 24px |
| `--space-xl` | 32px |

These are available as CSS variables but are not registered as Tailwind utilities -- use Tailwind's built-in spacing scale (`p-1`, `gap-4`, etc.) for most layout.

### Body Background

The dark theme applies a fixed multi-layer radial gradient background over the solid `#000000` base, creating subtle purple/blue nebula-like depth:

```css
background-image:
  radial-gradient(ellipse 70% 50% at 5% 95%, rgba(120, 80, 220, 0.22) 0%, transparent 70%),
  radial-gradient(ellipse 55% 45% at 95% 5%, rgba(40, 120, 200, 0.18) 0%, transparent 65%),
  radial-gradient(ellipse 60% 50% at 50% 50%, rgba(80, 60, 160, 0.1) 0%, transparent 55%),
  radial-gradient(ellipse 45% 35% at 80% 80%, rgba(60, 140, 180, 0.13) 0%, transparent 55%),
  radial-gradient(ellipse 40% 30% at 20% 20%, rgba(100, 60, 180, 0.15) 0%, transparent 50%);
background-attachment: fixed;
```

### Focus Ring

All interactive elements use a subtle brand-tinted outline on `:focus-visible`:

```css
outline: 2px solid color-mix(in srgb, var(--brand) 40%, transparent);
outline-offset: 2px;
```

## 7. Animation

### Streaming Cursor

A blinking cursor appended via `::after` to the last element inside a `.streaming-cursor` container:

```css
.streaming-cursor > div > *:last-child::after {
  width: 2px; height: 16px;
  background: var(--brand);
  animation: cursor-blink 1s ease-in-out infinite;
}
```

### Keyframe Animations

| Animation | Description |
|---|---|
| `cursor-blink` | Blinks opacity 0.8 to 0.15 over 1s |
| `distraction-pulse` | Subtle opacity pulse (0.07 to 0.12) for distraction overlay |
| `menu-appear` | Scale 0.96 to 1 with fade-in for context menus |
| `fade-in` | Fade in with 4px upward slide |
| `glass-appear` | Scale 0.97 to 1 with 6px upward slide for glass panels |
| `nudge-slide-in` | 8px upward slide for notification nudges |
| `breathe` | Opacity 0.5 to 1 breathing effect |
| `fade-in-up` | Fade in with 6px upward slide (variant) |

### Animation Durations (from _base.css)

| Variable | Value |
|---|---|
| `--duration-fast` | 0.15s |
| `--duration-normal` | 0.2s |
| `--duration-slow` | 0.3s |

### Reduced Motion

All animations and transitions are suppressed when the user prefers reduced motion:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

## 8. Themes

### Dark Theme (Default)

Applied via `[data-theme="dark"]` or bare `:root`. Uses the glassmorphism aesthetic described throughout this document: translucent surfaces, backdrop-blur, subtle borders, deep black background with nebula gradients.

### Retro Theme

Applied via `[data-theme="retro"]`. A complete visual departure:

- **No glassmorphism:** All `backdrop-filter` is disabled globally (`backdrop-filter: none !important`).
- **Opaque surfaces:** Warm cream/paper colors (`#f5f0e1`, `#ebe6d5`, `#e8e2cf`).
- **Chunky borders:** 2px solid `#2a2520` (dark brown) replacing translucent glass borders.
- **Hard drop shadows:** `3px 3px 0px #2a2520` pixel-style shadows instead of blur shadows.
- **Small radii:** `--radius: 4px` (vs dark theme's `12px`), `--radius-pill: 4px` (square pills).
- **Typography:** "Space Grotesk" font stack.
- **Checkerboard background:** SVG pattern tile instead of radial gradients.
- **Push-button interactions:** Active state uses `transform: translate(1px, 1px)` for physical push feel.
- **Win95-style scrollbars:** 14px wide, chunky thumb, visible track borders.

All glass classes are overridden per-component in `retro.css` to replace blur with solid opaque equivalents while preserving the same class API.

### Theme Switching

The `data-theme` attribute on the root element controls which theme is active. The `ThemeProvider` component in `src/app/providers/ThemeProvider.tsx` manages this. Personalization settings allow users to switch themes.

## 9. Guidelines

These rules are enforced by convention and documented in `CLAUDE.md`:

1. **Never hardcode hex/rgba values in components.** Always use token-based Tailwind utilities (`bg-surface-base`, `text-muted`, `border-border`). This ensures themes work correctly.

2. **Never write raw `backdrop-filter: blur() saturate()` in CSS.** The CSS minifier breaks compound `backdrop-filter` declarations. Always use Tailwind's `@apply backdrop-blur-* backdrop-saturate-*` in utility classes.

3. **Parent `backdrop-blur` blocks child `backdrop-filter`.** Be aware that nesting glass elements can cause children to lose their blur effect.

4. **Never use `overflow-x-auto` or `overflow: hidden` on containers with absolute dropdown children.** The overflow clipping will cut off dropdown menus. Use React portals to render dropdowns outside the overflow container instead.

5. **For new visual patterns, add a CSS variable to `:root` first,** register it in `@theme inline`, then use via Tailwind. Do not use one-off inline styles.

6. **Use the `cn()` utility** for conditional class composition. It combines `clsx` (conditional logic) with `tailwind-merge` (deduplication of conflicting Tailwind classes).

7. **Timestamps are UTC from the backend.** Never `.slice()` ISO strings for display. Always parse via `new Date(iso)` and use locale-aware formatters. Use the shared `formatTime()` helper from `src/shared/lib/dates.ts`.

## 10. Adding New Tokens

To add a new design token to the system:

### Step 1: Define the CSS Variable

Add the variable to the appropriate theme file(s). For a color that should change per-theme, add it to both `dark.css` and `retro.css`:

```css
/* src/shared/styles/themes/dark.css */
[data-theme="dark"], :root {
  --my-new-color: rgba(100, 200, 150, 0.3);
}

/* src/shared/styles/themes/retro.css */
[data-theme="retro"] {
  --my-new-color: #5cb87a;
}
```

For structural tokens (spacing, radius, etc.) that are theme-agnostic, add them to `_base.css`.

### Step 2: Register in @theme inline

Add a `--color-*` entry to the `@theme inline` block in the theme file(s):

```css
@theme inline {
  --color-my-new-color: var(--my-new-color);
}
```

This must be done in each theme file that has a `@theme inline` block (`dark.css`, `retro.css`, and the legacy `theme.css`).

### Step 3: Use in Components

The new token is now available as a Tailwind utility:

```tsx
<div className="bg-my-new-color text-primary border-my-new-color">
  ...
</div>
```

For non-color tokens (spacing, shadows, etc.), use `var(--my-token)` in CSS or register under the appropriate Tailwind theme namespace.
