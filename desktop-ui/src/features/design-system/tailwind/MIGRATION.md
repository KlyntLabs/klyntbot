# Tailwind CSS v4 Migration Guide

## What Changed

Tailwind CSS v4 is now available **globally** across the entire app (not just `src/tracing/`).

### New Files

| File | Purpose |
|------|---------|
| `src/styles/tailwind-theme.css` | Bridges all existing CSS custom properties into Tailwind's `@theme inline` system |
| `src/features/design-system/tailwind/` | CVA-based design system primitives (Button, Badge, Input, Card, Panel) |

### Modified Files

| File | Change |
|------|--------|
| `src/styles/index.css` | Added `@import "./tailwind-theme.css"` at the top |
| `src/features/app/components/MainTopbar.tsx` | Converted from BEM classes to Tailwind utilities |
| `src/features/design-system/components/panel/PanelPrimitives.tsx` | Converted from BEM classes to Tailwind utilities |
| `src/features/settings/components/SettingsShell.tsx` | Replaced arbitrary values with semantic tokens |

## Available Utilities

Because the theme bridges your existing CSS custom properties, **theme switching still works automatically**. When `data-theme` changes on `<html>`, all Tailwind utilities update.

### Colors

```tsx
// Surfaces
"bg-surface-sidebar"
"bg-surface-messages"
"bg-surface-card"
"bg-surface-active"
"bg-surface-hover"

// Text
"text-text-primary"
"text-text-strong"
"text-text-muted"
"text-text-faint"
"text-text-accent-cyan"

// Borders
"border-border-subtle"
"border-border-strong"
"border-border-accent"

// Status
"text-status-success"
"text-status-error"
"bg-status-warning"

// CodeMirror chips
"bg-cm-blue-bg"
"text-cm-blue-fg"
"border-cm-blue-border"

// Tool families
"text-tool-filesystem"
"text-tool-shell"
```

### Typography

```tsx
"text-ui-3xs"   // 9px
"text-ui-2xs"   // 10.5px
"text-ui-xs"    // 11.5px
"text-ui-sm"    // 12.5px
"text-ui-md"    // 13.5px
"text-ui-lg"    // 15px
"text-ui-xl"    // 17px
"font-family-ui"
"font-family-code"
```

### Spacing

```tsx
"p-ui-0-5"   // 2px
"p-ui-1"     // 4px
"gap-ui-2"   // 8px
"m-ui-3"     // 12px
"space-ui-4" // 16px
```

### Motion

```tsx
"duration-ui-fast"     // 120ms
"duration-ui-normal"   // 160ms
"duration-ui-slow"     // 220ms
"ease-ui-out"          // cubic-bezier(0.16, 1, 0.3, 1)
"ease-ui-spring"       // cubic-bezier(0.34, 1.56, 0.64, 1)
"animate-ui-spin"
"animate-ui-fade-in"
```

### Radius

```tsx
"rounded-ui-sm"   // 6px
"rounded-ui-md"   // 8px
"rounded-ui-lg"   // 10px
"rounded-ui-xl"   // 14px
"rounded-ui-full"
```

### Dark Mode

Use `dark:` prefix for overrides. Both `data-theme="dark"` and `data-theme="dim"` trigger it:

```tsx
<div className="bg-surface-card dark:bg-surface-card-strong" />
```

## Design System Primitives

Import from the new Tailwind design system:

```tsx
import { Button, Badge, Input, Card, SearchField, PanelNavItem } from "@/features/design-system/tailwind";

<Button variant="primary" size="lg">Save</Button>
<Button variant="ghost" size="icon"><Icon /></Button>

<Badge variant="success">Connected</Badge>
<Badge variant="error" size="sm">Failed</Badge>

<SearchField icon={<SearchIcon />} placeholder="Search..." />

<Card>
  <CardHeader>
    <CardTitle>Settings</CardTitle>
    <CardDescription>Manage your preferences</CardDescription>
  </CardHeader>
  <CardContent>...</CardContent>
</Card>
```

## Migration Strategy

### Phase 1: New Components (Now)
Write all new components with Tailwind utilities + `cn()`.

### Phase 2: Refactor on Touch (Ongoing)
When you edit a component, convert its BEM classes to Tailwind utilities:

**Before:**
```tsx
<div className={`main-topbar ${className}`}>
  <div className="main-topbar-left">{leftNode}</div>
  <div className="actions">{actionsNode}</div>
</div>
```

**After:**
```tsx
<div className={cn(
  "flex items-center justify-between gap-3",
  "h-[var(--main-topbar-height,44px)]",
  "border-b border-border-subtle bg-surface-topbar",
  className
)}>
  <div className="flex items-center gap-2 min-w-0">{leftNode}</div>
  <div className="flex items-center gap-1 shrink-0">{actionsNode}</div>
</div>
```

### Phase 3: Remove Legacy CSS (Later)
As a component's CSS file is fully migrated, delete or trim it from `src/styles/index.css`.

## Rules of Thumb

1. **Use `cn()` for all className composition** — it handles conditional classes and resolves Tailwind conflicts.
2. **Prefer semantic tokens** — `bg-surface-card` not `bg-[rgba(255,255,255,0.04)]`.
3. **Keep layout CSS in CSS files** — Complex grid layouts like `.app` and `.sidebar-resizer` are poorly suited to utilities. Leave them in CSS until you have a reason to move them.
4. **Don't delete legacy CSS until all consumers are migrated** — The hybrid approach is safe; both systems work together.
5. **React 19: ref is a prop** — No `forwardRef` needed.

## Utility Patterns

### Focus Ring
```tsx
"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-border-accent focus-visible:ring-offset-2"
```

### Truncate
```tsx
"truncate" // overflow-hidden text-ellipsis whitespace-nowrap
```

### Flex Center
```tsx
"flex items-center justify-center"
```

### Disabled State
```tsx
"disabled:opacity-50 disabled:cursor-not-allowed"
```

### Reduced Motion
```tsx
"motion-reduce:transition-none motion-reduce:animate-none"
```

## Troubleshooting

### "Utility class not working"
Make sure the CSS variable is bridged in `src/styles/tailwind-theme.css`. Add it to `@theme inline` if missing.

### "Colors don't update on theme switch"
Verify you're using the bridged token (e.g. `text-text-primary`) not a hardcoded value. The bridge references CSS vars which update dynamically.

### "Tracing app looks wrong"
The tracing app already had its own Tailwind setup. The global theme now extends it. If you see conflicts, the tracing-specific `@theme inline` in `src/tracing/styles/tracing.css` takes precedence for tracing-scoped components because of CSS cascade.
