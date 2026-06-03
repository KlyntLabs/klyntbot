# Klynt Design System — Tailwind CSS v4

## Architecture

### Unified CSS Entry Point

```
src/styles/
├── app.css                          ← Main entry (Tailwind + all styles)
├── themes.css                       ← Imports primitives, semantic, themes
├── ds-tokens.css                    ← Component-specific aliases
├── tokens/                          ← NEW: OKLCH-based token system
│   ├── primitives.css               ← Base values (hue, chroma, lightness)
│   ├── semantic.css                 ← Derived tokens (bg, fg, border, accent)
│   ├── colors-extended.css          ← Status, CM chips, tool families
│   ├── typography.css               ← Font families, size scale
│   ├── spacing.css                  ← Spacing scale
│   ├── motion.css                   ← Durations, easings
│   └── radius.css                   ← Border radius scale
└── themes/                          ← NEW: Theme files (only override primitives)
    ├── dark.css                     ← Dark theme + backward-compat aliases
    ├── dim.css                      ← Dim theme
    ├── light.css                    ← Light theme
    └── system.css                   ← prefers-color-scheme wrapper
```

### The 3-Layer Token System

```
Layer 1: PRIMITIVES        Layer 2: SEMANTIC           Layer 3: COMPONENT
─────────────────          ─────────────────           ──────────────────
--primitive-bg-l: 15%  →   --color-surface             --composer-bg
--primitive-fg-l: 95%  →   --color-foreground          --sidebar-bg
--primitive-accent-hue   →   --color-accent              --message-bubble-bg
                           --color-border
                           --color-success
                           --color-error
```

**Key insight**: Themes only override ~6 primitives. All semantic tokens derive automatically via `oklch()` and `color-mix()`.

---

## Semantic Tokens (New — Use These)

### Backgrounds

| Token | Utility | Usage |
|-------|---------|-------|
| `--color-surface` | `bg-surface` | App background, base layer |
| `--color-surface-elevated` | `bg-surface-elevated` | Cards, panels, topbar |
| `--color-surface-sunken` | `bg-surface-sunken` | Inputs, controls, code blocks |
| `--color-surface-hover` | `bg-surface-hover` | Hover states |
| `--color-surface-active` | `bg-surface-active` | Active/selected states |
| `--color-overlay` | `bg-overlay` | Modals, popovers, toasts |
| `--color-inset` | `bg-inset` | Deeply nested content |

### Foregrounds (Text)

| Token | Utility | Usage |
|-------|---------|-------|
| `--color-foreground` | `text-foreground` | Primary text |
| `--color-foreground-muted` | `text-foreground-muted` | Secondary text, labels |
| `--color-foreground-faint` | `text-foreground-faint` | Placeholders, hints |
| `--color-foreground-accent` | `text-foreground-accent` | Links, active text |
| `--color-foreground-on-accent` | `text-foreground-on-accent` | Text on accent buttons |

### Borders

| Token | Utility | Usage |
|-------|---------|-------|
| `--color-border` | `border-border` | Default borders |
| `--color-border-strong` | `border-border-strong` | Focused, emphasized borders |
| `--color-border-accent` | `border-border-accent` | Active/brand borders |

### Accent & Status

| Token | Utility | Usage |
|-------|---------|-------|
| `--color-accent` | `bg-accent`, `text-accent` | Primary accent color |
| `--color-accent-strong` | `bg-accent-strong` | Hover/active accent |
| `--color-accent-muted` | `bg-accent-muted` | Subtle accent backgrounds |
| `--color-success` | `text-success`, `bg-success` | Success states |
| `--color-warning` | `text-warning`, `bg-warning` | Warning states |
| `--color-error` | `text-error`, `bg-error` | Error states |

---

## Quick Reference

### Layout Primitives

```tsx
import { Box, Stack, HStack, VStack, Grid, Container } from "@/features/design-system/tailwind";

<Box className="p-4">...</Box>
<Stack gap="4" align="center">...</Stack>
<HStack gap="2" justify="between">...</HStack>
<Grid cols={3} gap="4">...</Grid>
<Container size="lg">...</Container>
```

### Typography

```tsx
import { Text, Heading } from "@/features/design-system/tailwind";

<Text size="sm" color="muted" truncate>Hello</Text>
<Heading as="h2" size="lg" color="strong">Title</Heading>
```

### Surfaces

```tsx
import { Surface, Divider } from "@/features/design-system/tailwind";

<Surface variant="card" radius="lg" border>
  <Divider color="subtle" spacing="2" />
</Surface>
```

### Forms

```tsx
import { Label, Input, Textarea, Switch } from "@/features/design-system/tailwind";

<Label required>Email</Label>
<Input placeholder="Type here..." />
<Textarea error="Invalid input" />
<Switch label="Enable feature" />
```

### Feedback

```tsx
import { Skeleton, Spinner } from "@/features/design-system/tailwind";

<Skeleton variant="text" width="120px" />
<Spinner size="md" color="accent" />
```

### Data Display

```tsx
import { Avatar, Chip, Badge } from "@/features/design-system/tailwind";

<Avatar size="md" alt="John Doe" />
<Chip variant="success" size="sm">Active</Chip>
<Badge variant="primary">New</Badge>
```

---

## Backward Compatibility

Old tokens (`--text-muted`, `--surface-card`, `--border-subtle`, etc.) still work.
They are aliased to the new semantic tokens in each theme file:

```css
/* themes/dark.css */
--text-muted: var(--color-foreground-muted);
--surface-card: var(--color-surface-elevated);
--border-subtle: var(--color-border);
```

This means:
- ✅ Legacy BEM CSS in `@layer components` continues to work
- ✅ Existing components using old Tailwind utilities (`text-text-muted`) still work
- ✅ New components should use semantic tokens (`text-foreground-muted`)

---

## Custom Utilities

```tsx
// Buttons (legacy compat)
className="btn-primary"
className="btn-secondary"
className="btn-ghost"

// Layout
className="flex-center"
className="flex-between"
className="truncate-text"
className="no-drag"
className="no-select"

// Animations
className="animate-spin-slow"
className="animate-fade-in"
className="animate-slide-in"
```

---

## Theme Switching

Themes work via `data-theme` on `<html>`:

```tsx
document.documentElement.dataset.theme = "dark" | "dim" | "light";
```

Or follow system preference (no data-theme attribute):

```css
/* System theme uses prefers-color-scheme */
@media (prefers-color-scheme: light) { ... }
```

### Adding a New Theme

Create a new file in `src/styles/themes/`:

```css
/* themes/ocean.css */
[data-theme="ocean"] {
  color-scheme: dark;
  --primitive-bg-l: 15%;
  --primitive-fg-l: 95%;
  --primitive-accent-hue: 190;    /* Teal instead of blue */
  --primitive-accent-chroma: 0.16;
}
```

Import it in `themes.css`:

```css
@import "./themes/ocean.css";
```

Done. All semantic tokens derive automatically.

---

## Dark Mode

Use `dark:` prefix for theme overrides. Both `data-theme="dark"` and `data-theme="dim"` trigger it:

```tsx
<div className="bg-surface dark:bg-surface-elevated" />
```

---

## Design Token Philosophy

### Old Way (RGBA — 80+ tokens)
```css
--surface-card: rgba(255, 255, 255, 0.04);
--surface-card-strong: rgba(255, 255, 255, 0.12);
--surface-card-muted: rgba(255, 255, 255, 0.06);
--text-muted: rgba(255, 255, 255, 0.7);
--text-faint: rgba(255, 255, 255, 0.5);
/* ... 80 more */
```

### New Way (OKLCH — 18 tokens)
```css
--primitive-bg-l: 15%;
--primitive-fg-l: 95%;

--color-surface: oklch(var(--primitive-bg-l) 0.01 255);
--color-surface-elevated: oklch(calc(var(--primitive-bg-l) + 5%) 0.01 255);
--color-foreground: oklch(var(--primitive-fg-l) 0.01 255);
--color-foreground-muted: color-mix(in oklch, var(--color-foreground) 55%, var(--color-surface));
```

**Why OKLCH?**
- Perceptually uniform — changing lightness doesn't shift hue
- `color-mix(in oklch, ...)` produces natural tints/shades
- Changing `--primitive-accent-hue` shifts the entire app accent
- 96%+ browser support

---

## Migration Path

| Step | Action |
|------|--------|
| 1 | Use new semantic tokens in new components |
| 2 | Refactor existing components on touch |
| 3 | Remove old `@theme inline` bridges when no consumers remain |
| 4 | Eventually remove old token aliases from theme files |
