# Translation Language Config Redesign

## Problem

The current translation flow requires users to select a target language from a context menu submenu every time they translate. This is:
- **Repetitive** — the target language rarely changes within a note
- **Not keyboard-friendly** — submenus don't work with shortcuts or future Vim mode
- **Missing source language** — no clear definition of the user's primary language

## Design

### 1. Language Config Section in AI Suggestions Panel

Add a new collapsible **"Language"** section at the top of `AISuggestionsPanel` (above Related Notes). Contains two compact dropdown selectors side-by-side:

```
LANGUAGE
┌──────────────┐  ┌──────────────┐
│ 🇻🇳 Tiếng Việt ▾│  │ 🇬🇧 English ▾ │
│   Source      │  │   Target     │
└──────────────┘  └──────────────┘
```

- Each dropdown shows `flag + native name` for each language
- A `text-[9px]` label below each reads "Source" / "Target"
- On change, calls `setLanguagePair({ sourceLang, targetLang })` from `usePerspective` to persist to the note's `perspectiveConfig`
- Defaults for new notes (no `perspectiveConfig` yet): `🇻🇳 Tiếng Việt` source → `🇬🇧 English` target, resolved from global config fallback in `useLanguageConfig`

### 2. Simplified Context Menu

The "Translate" item becomes a **direct action** — single click, no submenu:

```
┌─ SELECTION ──────────┐
│  Annotate        ⌥A  │
│  Create Flashcard ⌥F │
│  Translate            │  ← single click, uses pre-configured languages
├─ AI ACTIONS ─────────┤
│  Ask AI               │
└───────────────────────┘
```

Removes `TRANSLATE_LANGUAGES` submenu from `EditorContextMenu`. The language pair is already resolved by `useLanguageConfig` from the note's `perspectiveConfig`.

### 3. Data Flow

No backend changes. The flow:

```
Panel dropdown change
  → setLanguagePair({ sourceLang, targetLang })     // usePerspective (existing)
  → ipc("note_update", { perspectiveConfig: ... })  // persists to SQLite

Context menu "Translate" click
  → useLanguageConfig reads perspectiveConfig        // already resolved
  → triggerTranslateText(text, rect)                 // useQuickTranslate (existing)
  → ipc("language_quick_translate", { text, sourceLang, targetLang })
```

## Changes

### Files to modify

1. **`AISuggestionsPanel.tsx`** — Add "Language" section with two dropdown selectors. New props: `sourceLang`, `targetLang`, `onLanguageChange(field, code)`.

2. **`EditorContextMenu.tsx`** — Remove `TRANSLATE_LANGUAGES` array and submenu. "Translate" becomes a `MenuItem` calling `onTranslate(selectedText, rect)`. Remove `onTranslateTo` prop and `noteTargetLang` prop.

3. **`NoteEditor.tsx`** — Pass `sourceLang`/`targetLang`/`onLanguageChange` to `AISuggestionsPanel`. Simplify `handleTranslateTo` → `handleTranslate` (no lang param, uses already-resolved language pair). Remove `onTranslateTo` from context menu wiring.

4. **New shared constant** — Move `TRANSLATE_LANGUAGES` to a shared constants file (e.g., `@shared/constants/languages.ts`) with flag emojis added. Used by both `AISuggestionsPanel` dropdowns and any future language selection UI.

### Files unchanged

- `useLanguageConfig.ts` — already resolves per-note overrides from `perspectiveConfig`
- `usePerspective.ts` — `setLanguagePair()` already persists to `perspectiveConfig`
- `useQuickTranslate.ts` — already receives `sourceLang`/`targetLang` as params
- `QuickTranslatePopup.tsx` — no changes
- All Rust backend — no changes

## Language List

Shared constant with flag emojis:

| Code | Flag | Native |
|------|------|--------|
| zh | 🇨🇳 | 中文 |
| ja | 🇯🇵 | 日本語 |
| ko | 🇰🇷 | 한국어 |
| vi | 🇻🇳 | Tiếng Việt |
| en | 🇬🇧 | English |
| es | 🇪🇸 | Español |
| fr | 🇫🇷 | Français |
| de | 🇩🇪 | Deutsch |
| ru | 🇷🇺 | Русский |
| ar | 🇸🇦 | العربية |
| th | 🇹🇭 | ไทย |
| hi | 🇮🇳 | हिन्दी |
| pt | 🇧🇷 | Português |

## Non-goals

- No changes to the translation LLM pipeline or prompts
- No new IPC commands
- No changes to `perspectiveConfig` schema (already supports `languagePair`)
- No global settings page — global defaults come from existing `config.json` → `language` section
