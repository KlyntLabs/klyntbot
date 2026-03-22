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
- On change, saves to the note's `perspectiveConfig` via `ipc("note_update")` — same mechanism `usePerspective.setLanguagePair()` uses
- Defaults for new notes (no `perspectiveConfig` yet): whatever `useLanguageConfig` resolves from global config → auto-detect → fallback. For a user with `nativeLang: "vi"` and `targetLang: "en"` in global config, this shows `🇻🇳 Tiếng Việt → 🇬🇧 English`

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

### 3. Component Tree & Data Flow

```
KnowledgeBasePage
  ├── NoteEditorPanel → NoteEditor
  │     ├── EditorContextMenu  ← "Translate" (single click, uses resolved lang pair)
  │     ├── useLanguageConfig(perspectiveConfig)  ← resolves sourceLang/targetLang
  │     ├── usePerspective  ← setLanguagePair() for context menu (existing)
  │     └── useQuickTranslate  ← triggerTranslateText()
  └── ContextPanel
        └── AISuggestionsPanel  ← NEW: language dropdowns
              ├── useLanguageConfig(perspectiveConfig)  ← resolves current pair for display
              └── ipc("note_update", { perspectiveConfig })  ← saves changes directly
```

**Key insight:** `AISuggestionsPanel` lives in `ContextPanel` (sibling of `NoteEditor`, not a child). Instead of threading callbacks up through `KnowledgeBasePage`, the panel handles language persistence internally:

1. `ContextPanel` already receives `note: Note | null` — pass `perspectiveConfig` down to `AISuggestionsPanel`
2. `AISuggestionsPanel` calls `useLanguageConfig(perspectiveConfig)` to resolve current languages for display
3. On dropdown change, `AISuggestionsPanel` saves directly via `ipc("note_update", { id: noteId, params: { perspectiveConfig: updatedJson } })` — the same pattern `usePerspective` uses
4. `NoteEditor`'s `useLanguageConfig` picks up the change on next render via the SWR cache invalidation from `note_update`

No backend changes needed.

## Changes

### Files to modify

1. **`AISuggestionsPanel.tsx`** — Add "Language" section with two dropdown selectors. New prop: `perspectiveConfig: string | null`. Calls `useLanguageConfig` internally. Saves language changes via direct `ipc("note_update")`.

2. **`ContextPanel.tsx`** — Pass `note?.perspectiveConfig ?? null` (from existing `note` prop) to `AISuggestionsPanel`.

3. **`EditorContextMenu.tsx`** — Remove `TRANSLATE_LANGUAGES` array and the `<ContextMenu.Sub>` submenu block. "Translate" becomes a plain `MenuItem` calling existing `onTranslate(selectedText, rect)`. Remove `onTranslateTo` prop and `noteTargetLang` prop.

4. **`NoteEditor.tsx`** — Remove `handleTranslateTo` and its wiring to context menu. The existing `handleTranslate` already does what's needed. Remove `onTranslateTo` and `noteTargetLang` from `EditorContextMenu` props.

5. **New shared constant** — Create `@shared/constants/languages.ts` (new `constants/` directory) with `LANGUAGES` array containing `{ code, label, native, flag }`. Reused by `AISuggestionsPanel` dropdowns.

### Files unchanged

- `useLanguageConfig.ts` — already resolves per-note overrides from `perspectiveConfig`
- `usePerspective.ts` — still used by `NoteEditor` for other perspective operations
- `useQuickTranslate.ts` — already receives `sourceLang`/`targetLang` as params
- `QuickTranslatePopup.tsx` — no changes
- `KnowledgeBasePage.tsx` — no changes (data already flows through existing props)
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
| pt | 🇵🇹 | Português |

## Non-goals

- No changes to the translation LLM pipeline or prompts
- No new IPC commands
- No changes to `perspectiveConfig` schema (already supports `languagePair`)
- No global settings page — global defaults come from existing `config.json` → `language` section
