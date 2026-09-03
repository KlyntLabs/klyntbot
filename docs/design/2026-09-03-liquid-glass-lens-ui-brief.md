# UI brief — KlyntBot design system for macOS Tahoe: "Lens"

Status: Locked (draft-ui pick, 2026-09-03)  
Scope: the `--ds-*` token system and the shell chrome every screen inherits (rail, chats panel, conversation column, composer). Screen-specific surfaces (Dashboard, Tasks, Settings, HUD windows) adopt these tokens; they are not re-decided here.  
Screenshots: [`…-light.png`](./2026-09-03-liquid-glass-lens-ui-brief-light.png) · [`…-dark.png`](./2026-09-03-liquid-glass-lens-ui-brief-dark.png)

## Decision

**Variant B — "Lens"** wins outright over A (Tahoe, first-party fidelity), C (Graphite, dense pro tool), and H (A + B's composer + B's dark ground). In the user's words: *"I think B - Lens, is work with me."* Picked as shown; no amendments requested during review.

Reviewed against: variant 0 (the current system rebuilt on the same content) so the change is judged, not imagined.

## Grounding

1. User's words: "re-design the color, system … match with the new era … desktop app, work only in macOS … newest design like liquid glass".
2. Existing system: `packages/design-system/src/tokens/*.css` (`--ds-*` namespace, role-based scales), `docs/standards/design-tokens.md` (Approved 2026-09-02), `docs/standards/ui.md`. The `--ds-*` vocabulary, the 4px space scale, the type-step names, the radius-by-role rule, the z-bands, and the light | dark theme model **stay**. This brief changes values, adds a small set of tokens, and retires one.
3. Apple Liquid Glass (macOS 26): glass is the control/navigation layer floating above content, never the content layer; regular glass for text-heavy surfaces; sidebars inset with content flowing beneath; concentric nested radii; large controls are capsules; tint one primary action; bolder type; scroll-edge effects instead of dividers; hierarchy by layout and grouping, not borders. Sources listed at the end.

## Token changes (source of record for the build)

All values below replace or extend `packages/design-system/src/tokens/`. Light is `:root`; dark is `html[data-theme="dark"]`.

### color.css

| token | light | dark | note |
|---|---|---|---|
| `--ds-bg` | `#f5f4f8` | `#0f0f14` | porcelain / near-black; **never pure `#000`** |
| `--ds-bg-elevated` | `#ffffff` | `#1a1a22` | content-layer cards, task lists |
| `--ds-text` | `#141420` | `rgba(255,255,255,.93)` | |
| `--ds-text-secondary` | `rgba(20,20,40,.58)` | `rgba(255,255,255,.60)` | |
| `--ds-text-dim` | `rgba(20,20,40,.34)` | `rgba(255,255,255,.34)` | |
| `--ds-control-hover` | `rgba(20,20,40,.06)` | `rgba(255,255,255,.08)` | neutral wash (unchanged role) |
| `--ds-control-active` | `rgba(20,20,40,.11)` | `rgba(255,255,255,.14)` | |
| `--ds-separator` | `rgba(20,20,40,.09)` | `rgba(255,255,255,.10)` | content layer only; chrome uses no 1px borders |
| `--ds-accent` | `#5b6cff` | `#7b88ff` | periwinkle — spent on Send and the active rail item only |
| `--ds-accent-hover` | `#4a5be8` | `#8f9aff` | |
| `--ds-accent-tint` **new** | `rgba(91,108,255,.12)` | `rgba(123,136,255,.18)` | user-message fill, active-rail fill |
| `--ds-glass-bg` | `rgba(255,255,255,.55)` | `rgba(28,28,36,.55)` | regular glass |
| `--ds-glass-highlight` **new** | `rgba(255,255,255,.80)` | `rgba(255,255,255,.16)` | 1px inner top highlight |
| `--ds-glass-edge` **new** | `rgba(255,255,255,.55)` | `rgba(255,255,255,.14)` | lens edge gradient stop |
| `--ds-glass-shadow` | `rgba(40,40,90,.14)` | `rgba(0,0,0,.50)` | |
| `--ds-glass-border` | **retire** | | glass has no border; lens edge replaces it |
| `--ds-ambient-1/2/3` **new** | `#3a8bff` / `#8a5cf6` / `#5ad1b3` | same | aurora hues: accent blue, AI violet (= `--ds-origin-ai`), mint |
| `--ds-ambient-alpha` **new** | `.18` | `.30` | aurora strength |
| `--ds-status-success` | `#2fbf71` | `#3ddc84` | |
| `--ds-status-danger` | `#ff4d5e` | `#ff5c6b` | |
| `--ds-status-warning` | `#f5a524` | `#ffb340` | |
| `--ds-status-info` | `var(--ds-accent)` | | |
| `--ds-code-bg` | `rgba(20,20,40,.06)` | `rgba(255,255,255,.07)` | |

`--ds-glass-bg-strong` / `-subtle`, `--ds-glass-floating*`, `--ds-glass-dropdown`, `--ds-glass-context`, `--ds-glass-badge*`: derive from the new `--ds-glass-bg` at ±10% alpha during the build; HUD windows keep their vibrancy override.

### elevation.css

- `--ds-blur: blur(36px) saturate(1.9)`; `--ds-blur-strong: blur(44px) saturate(1.9)`; `--ds-blur-subtle` retired (compact controls are not glass).
- `--ds-elevation-glass: inset 0 1px 0 var(--ds-glass-highlight), 0 14px 44px var(--ds-glass-shadow)`.
- Lens edge is a recipe, not a token: a `::before` 1px gradient ring (`--ds-glass-highlight` → `--ds-glass-edge` 30% → transparent 50% → `--ds-glass-edge`), masked to the border box.

### radius.css (concentric chain)

`--ds-radius-capsule: 999px` (new role) · `--ds-radius-panel: 20px` · `--ds-radius-md: 12px` · `--ds-radius-row: 10px` · `--ds-radius-control: 10px`. Rule: nested radius = parent radius − padding; the panel (20) with 10px padding gives rows 10.

### typography.css

- `--ds-font-rounded: ui-rounded, "SF Pro Rounded", -apple-system, system-ui, sans-serif` **new** — native in WKWebView, no web font.
- `--ds-font-weight-light` **retired**. Scale is 400 / 500 / 600.
- Steps unchanged; `--ds-text-body` 14 for messages, `--ds-text-ui` 13 for chrome.

### size.css

`--ds-row-h: 32px` (rail buttons 38, composer 56, send 40).

## Surfaces

### App shell (window ground + title bar)

Layout: no content sheet; the whole window is one ground (`--ds-bg`) with the aurora beneath everything. Title bar 44px, transparent, traffic-light spacer 68px, wordmark centred, Brain orb trailing. Nothing else lives in the title bar.  
Components: rung 2 — `AppShell.tsx` title bar (keep drag region and spacer); rung 2 — `KlyntLogo`; `new (rung 7)` — `AmbientGround` (the aurora: three radial gradients on a fixed layer, `filter: blur(60px)`, 48s alternate drift). Depth: if it vanished, callers need only know "a fixed, non-interactive layer painted under `.win`" — a one-element interface.  
States: default; `prefers-reduced-motion` → drift off, gradients static; `.resizing` → aurora `will-change` dropped and glass blur suspended (existing pattern in `legacy-glass.css`).  
Type & color: wordmark `--ds-font-rounded` 600 `--ds-text-ui` `--ds-text-secondary`; orb gradient from `--ds-ambient-1` → `--ds-ambient-2`.  
A11y: aurora `aria-hidden`; contrast is measured against the *composited* ground (aurora at `.18`/`.30` keeps `--ds-text` ≥ 12:1 light, ≥ 14:1 dark on the rail and panel glass).

### Icon rail (floating capsule)

Layout: absolute, 12px from the left window edge, 56px below the top; 52px wide capsule, 8px vertical padding, 4px gap; buttons 38px circles. A second small capsule at the bottom-left holds System and Settings.  
Components: rung 2 — `Sidebar.tsx` (same items, badge, pulse); the `island` recipe is **replaced** by the `glass` recipe + lens edge for this surface.  
States: default `--ds-text-secondary`; hover `--ds-control-hover` + `--ds-text`; active `--ds-accent-tint` fill + `--ds-accent` glyph; Learn count pill `--ds-accent` fill, white 9px rounded 600; Brain pulse dot `--ds-status-warning` with 6px glow.  
Type & color: glyphs 18px stroke 1.6; count `--ds-font-rounded`.  
A11y: buttons keep `aria-label`; `:focus-visible` 2px `--ds-accent` ring, offset 2px; arrow-key roving focus stays as today.

### Chats panel (floating glass)

Layout: absolute, right of the rail (left 76px), 236px wide, 20px radius, 12/10px padding; header row "Chats" + new-chat icon button; groups Today / Yesterday / Projects / Earlier; rows 32px, 10px radius, 8px gap.  
Components: rung 2 — `ThreadList.tsx`, `ThreadButton.tsx`, `ThreadContextMenu.tsx`; rung 2 — DS `Button` ghost for the header action. Drops the three text actions at the top (New chat / Resume last / Chat settings) in favour of one icon action; Resume and Chat settings move to the context menu (rung 2 — `ThreadContextMenu`).  
States: default; hover `--ds-control-hover`; active `--ds-control-active` + 500 weight; rename inline (existing); empty "No chats yet" in `--ds-text-secondary`; overflow scrolls inside the panel with no visible divider.  
Type & color: header `--ds-font-rounded` 600 `--ds-text-title-sm`; group labels `--ds-font-rounded` 600 `--ds-text-ui-xs` uppercase, `.06em`, `--ds-text-dim`; rows `--ds-text-ui` 400 `--ds-text`; times `--ds-text-ui-xs` `--ds-text-dim`, tabular.  
A11y: rows are buttons; group labels are headings; contrast of `--ds-text-dim` on glass ≥ 4.5:1 in both themes (verified on the screenshots' composite).

### Conversation column

Layout: the column starts right of the panel (left 332px) and is a 640px measure centred in the remaining width; 26px gap between turns; 130px bottom padding so the last turn clears the composer. The thread title sits at the top of the column as a document title with the provider status beneath it; the old bordered header row is gone.  
Components: rung 2 — `ChatPage.tsx` header content moves into the column; rung 2 — `MessageList.tsx`, `MarkdownContent.tsx`; `glass-bubble` / `glass-bubble-user` classes **retired** (content is never glass).  
States: user turn right-aligned, `--ds-accent-tint` fill, 20px radius, max 78%; assistant turn plain text, full measure; streaming shows the existing thinking dots; error row `--ds-status-danger` text on `--ds-bg-elevated`; "scroll to latest" pill uses the `chip` style.  
Type & color: title `--ds-font-rounded` 600 `--ds-text-title-lg` `-.01em`; status `--ds-text-ui-sm` 500 `--ds-text-secondary` with a 7px `--ds-status-success` dot; body `--ds-text-body` 400, line-height 1.6.  
A11y: `aria-live="polite"` on the list (existing); user-turn contrast `--ds-text` on `--ds-accent-tint` ≥ 10:1.

### Content-layer blocks (task list, code, chips, badge)

Layout: task list is a 16px-radius card on `--ds-bg-elevated` at 70% over the ground, rows 40px, hairline `--ds-separator` between rows only; code block 12px radius on `--ds-code-bg`; chips are capsules on `--ds-control-hover`.  
Components: rung 2 — DS `Badge` (border removed, `--ds-font-rounded` 600); rung 2 — DS `Button`; rung 2 — `PlanProgress` / task rendering in `MessageList`; chips are the existing `tag-pill` recipe re-tokenised.  
States: task open / done (strike + `--ds-status-success` check); badge tones default / success / warning / brand.  
Type & color: task title `--ds-text-ui` 500; meta `--ds-text-ui-xs` `--ds-text-secondary`; code `--ds-font-mono` `--ds-text-ui-sm`.  
A11y: task check is decorative in chat (state also in the badge text); code blocks `user-select: text`.

### Composer (X-Large lens capsule)

Layout: absolute, 20px above the window bottom, centred in the column, max 640px, 56px tall capsule; attach (38px circle) · field · dictate (38px circle) · Send (40px accent circle).  
Components: rung 2 — `ChatInput.tsx`, `VoiceToggle.tsx`; the `glass-input` class is **retired**; the capsule uses the `glass` recipe + lens edge. Send is the one tinted primary on the screen.  
States: default; field focus shows no ring on the capsule (the caret is the focus signal; the capsule already floats) but the Send button and icon buttons keep `:focus-visible`; disabled Send `--ds-control-hover` + `--ds-text-secondary`; recording state on dictate uses `--ds-status-danger` glyph; multiline grows to 4 lines then scrolls.  
Type & color: placeholder `--ds-font-rounded` 500 `--ds-text-secondary` ("Ask Klynt anything…"); Send `--ds-accent` with `inset 0 1px 0 rgba(255,255,255,.35)` and a `--ds-accent` 45% glow.  
A11y: ⌘K, ↩ send, ⇧↩ newline unchanged; Send `aria-label`; capsule never traps focus.

## Signature

The ground moves, so the glass finally refracts something. Every floating control (rail, panel, composer) is regular Liquid Glass over a slowly drifting aurora made of the app's own hues, with a 1px lens-edge highlight instead of a border. Rounded type on titles and labels is the second, quieter tell.

## Amendments

None requested at review. Decided constraints carried from the draft:

- Aurora respects `prefers-reduced-motion` (static) and is suspended during window resize; a user-facing "Ambient background" toggle in Settings › Appearance is in scope for the build so the aurora can be turned off without losing the palette.
- Accepted risk (raised, and the user chose B anyway): one animated blurred layer under three `backdrop-filter` surfaces per window. The build measures it with `./scripts/run_chat_perf_gates.sh` before the toggle default is decided.
- HUD windows (launcher, tray, distraction-overlay, voice-orb) adopt the new glass values but **not** the aurora; the OS wallpaper is their ground.

## Hand-off

`design-solution` Step 2b lifts the `## Surfaces` slots above 1:1 into `## UI design`. The token table is the input for a `frame-change` on the design-system package; the draft pages (`draft-ui/`) are scaffolding and are deleted at cleanup, they are not promoted.

Sources: Apple HIG [Materials](https://developer.apple.com/design/human-interface-guidelines/materials), [Color](https://developer.apple.com/design/human-interface-guidelines/color), [Sidebars](https://developer.apple.com/design/human-interface-guidelines/sidebars), [Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars); WWDC25 [Get to know the new design system](https://developer.apple.com/videos/play/wwdc2025/356/).
