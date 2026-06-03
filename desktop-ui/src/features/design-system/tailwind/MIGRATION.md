# Klynt Design System — Tailwind CSS v4

## Architecture

All app styles are now consolidated into a single entry point:

```
src/styles/app.css          ← Unified CSS (Tailwind v4 + all legacy styles)
src/styles/themes.css       ← Theme imports (dark/dim/light/system)
src/styles/themes.*.css     ← Theme variable definitions
src/styles/ds-tokens.css    ← Design system token aliases
```

**Legacy `src/styles/index.css` and 50+ scattered CSS files have been removed.**

## File Structure

```
src/styles/
  app.css              ← One file to rule them all
  themes.css           ← Imports all theme files
  themes.dark.css      ← Dark theme tokens
  themes.dim.css       ← Dim theme tokens
  themes.light.css     ← Light theme tokens
  themes.system.css    ← System theme tokens
  ds-tokens.css        ← Design system aliases

src/features/design-system/tailwind/
  index.ts             ← Export all primitives
  box.tsx              ← Generic container
  stack.tsx            ← Stack, HStack, VStack
  grid.tsx             ← Grid, Container
  text.tsx             ← Text, Heading
  surface.tsx          ← Surface, Divider
  skeleton.tsx         ← Loading placeholder
  spinner.tsx          ← Loading spinner
  label.tsx            ← Form label
  textarea.tsx         ← Multi-line input
  switch.tsx           ← Toggle switch
  avatar.tsx           ← User avatar
  chip.tsx             ← Tag/chip
  button.tsx           ← Button (CVA)
  badge.tsx            ← Badge (CVA)
  input.tsx            ← Input, SearchField
  card.tsx             ← Card compound
  panel.tsx            ← Panel compound
```

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

## Custom Utilities

The unified CSS defines `@utility` directives for common patterns:

```tsx
// Buttons (legacy compat)
className="btn-primary"
className="btn-secondary"
className="btn-ghost"
className="btn-danger"
className="btn-link"
className="icon-button"

// Layout
className="flex-center"
className="flex-between"
className="truncate-text"
className="no-drag"
className="no-select"

// Surfaces
className="surface-card"
className="surface-hover-effect"
className="focus-ring"

// Animations
className="animate-spin-slow"
className="animate-fade-in"
className="animate-slide-in"
```

## Design Tokens

All CSS custom properties are bridged to Tailwind utilities:

| Token Type | Utility Prefix | Example |
|-----------|---------------|---------|
| Surfaces | `bg-surface-*` | `bg-surface-card` |
| Text | `text-text-*` | `text-text-muted` |
| Borders | `border-border-*` | `border-border-strong` |
| Status | `text-status-*` | `text-status-error` |
| Typography | `text-ui-*` | `text-ui-sm` |
| Spacing | `gap-ui-*` / `p-ui-*` | `gap-ui-2` |
| Radius | `rounded-ui-*` | `rounded-ui-lg` |
| Motion | `duration-ui-*` | `duration-ui-fast` |

## Theme Switching

Themes continue to work via `data-theme` on `<html>`:

```tsx
document.documentElement.dataset.theme = "dark" | "dim" | "light";
```

All Tailwind utilities referencing CSS custom properties update automatically.

## Migration Rules

1. **New components** → Use atomic primitives + Tailwind utilities
2. **Refactoring legacy** → Replace BEM classes with utilities or atomic components
3. **Custom patterns** → Add `@utility` in `app.css` if reused across components
4. **One-off styles** → Use `className={cn("...", className)}` with Tailwind utilities

## Dark Mode

Use `dark:` prefix for theme overrides. Both `data-theme="dark"` and `data-theme="dim"` trigger it:

```tsx
<div className="bg-surface-card dark:bg-surface-card-strong" />
```
