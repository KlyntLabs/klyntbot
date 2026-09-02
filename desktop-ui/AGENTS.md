# Agent guide — desktop-ui

Stack: React 19 + Vite SPA (Tauri 2 webview), Tailwind v4 + `@klyntbot/design-system`,
lucide, custom IPC hooks (`useQuery` / `useMutation`), Biome, Vitest. Feature-sliced
layout under `src/` (`app/`, `features/`, `shared/`).

Canonical rules live in `docs/standards/` — read before UI work:

- **UI conventions** (shell vs features, theming, brand vs wash, done gates):
  [`docs/standards/ui.md`](../docs/standards/ui.md)
- **Design tokens & UI stack** (token vocabulary, scales, forbidden patterns):
  [`docs/standards/design-tokens.md`](../docs/standards/design-tokens.md)
- **Performance** (budgets, splitting, interaction rules):
  [`docs/standards/frontend-performance.md`](../docs/standards/frontend-performance.md)
- **Upgrade gaps / wave plan**:
  [`docs/standards/jdbot-fe-upgrade-gaps.md`](../docs/standards/jdbot-fe-upgrade-gaps.md)

Commands (run from `desktop-ui/`):

`bun run dev` · `bun run typecheck` · `bun run lint` · `bun run test` ·
`bun run build` · `bun run check:tokens`

This file is a pointer, not a second source of truth — do not add rule bodies
here; extend the standards docs instead.
