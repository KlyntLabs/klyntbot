# Design-tokens migration review

Status: **ship-with-nits** (blockers/majors from prior pass closed)  
Date: 2026-09-02

## Verdict history

1. Initial reviewer pass: **needs-fixes**
2. After follow-up fixes + audit: **ship-with-nits**

## Closed findings

| Sev | Finding | Resolution |
|---|---|---|
| blocker | DistractionOverlay `var(--destructive)` / raw rose rgba | → `--ds-status-danger` + `color-mix` |
| major | Setup Inline* brand borders on fields | → control-hover + separator + neutral focus |
| major | Chat send buttons missing light fg | → `text-brand-foreground` |
| major | AutomationsPage `hover:text-brand hover:bg-brand` | → `hover:bg-brand/10` |
| major | Runtime `var(--brand)` / `var(--destructive)` in charts/lib | → `--ds-accent` / `--ds-status-*` |
| major | `bg-surface-*` leftovers | → `bg-bg` / `bg-control-hover` / `bg-glass-*` |
| major | Legacy class sweep (`muted-foreground`, `bg-card`, …) | → 0 matches in `desktop-ui/src` |

## Remaining nits (acceptable)

- `compat.css` still dual-publishes shadcn/Tahoe aliases — keep until a hard `rg` gate proves unused, then delete
- `glass.css` legacy class names (`glass-panel`, …) still used; recipes preferred for new code
- ThemeSwitcher preview swatches still use literal hex (carve-out)
- Some `text-white` on brand CTAs remain alongside `text-brand-foreground` (equivalent)
- Setup checkbox selected state uses `border-brand` intentionally
- `HARD=1` token check not flipped yet — soft gate via `bun run check:tokens`

## Verification performed

- `bun run test` — 77 passed (after review fixes)
- `bun run build` — success
- Browser: settings personalization light/dark; Default model input neutral gray; tasks page light theme renders islands correctly
- Console noise is API 500 / backend-down, not CSS

## Accent vs brand (locked)

| Utility | Meaning |
|---|---|
| `bg-brand` / `text-brand` | Primary blue CTA (`--ds-accent`) |
| `bg-accent` / wash | Neutral `--ds-control-hover` (do not paint inputs blue) |
