# Translation Language Config Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move translation language selection from the context menu submenu into the AI Suggestions panel as two always-visible dropdowns, making "Translate" a single-click action.

**Architecture:** Pure frontend UI reorganization. `AISuggestionsPanel` gets a new "Language" section with source/target dropdowns that persist to `perspectiveConfig` via `ipc("note_update")`. `EditorContextMenu` loses its language submenu. No backend changes.

**Tech Stack:** React, TypeScript, Tailwind CSS, Radix UI (context menu), Tauri IPC

**Spec:** `docs/superpowers/specs/2026-03-22-translation-language-config-redesign.md`

---

### Task 1: Add `perspectiveConfig` to TypeScript types

**Files:**
- Modify: `desktop-ui/src/shared/types/notes.ts:3-18` (Note interface)
- Modify: `desktop-ui/src/shared/types/notes.ts:66-78` (NoteUpdateParams interface)

- [ ] **Step 1: Add `perspectiveConfig` to `Note` interface**

In `desktop-ui/src/shared/types/notes.ts`, add after line 15 (`splitMode`):

```typescript
  perspectiveConfig: string | null;
```

- [ ] **Step 2: Add `perspectiveConfig` to `NoteUpdateParams` interface**

In the same file, add after line 77 (`splitMode`):

```typescript
  perspectiveConfig?: string | null;
```

- [ ] **Step 3: Verify no type errors introduced**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | head -30`

Expected: may show errors in `NoteEditor.tsx` where the unsafe casts now conflict — that's fine, we fix those in Task 4.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/types/notes.ts
git commit -m "feat(notes): add perspectiveConfig to Note and NoteUpdateParams types"
```

---

### Task 2: Create shared LANGUAGES constant

**Files:**
- Create: `desktop-ui/src/shared/constants/languages.ts`

- [ ] **Step 1: Create the constants directory and file**

Create `desktop-ui/src/shared/constants/languages.ts`:

```typescript
export interface Language {
  code: string;
  label: string;
  native: string;
  flag: string;
}

export const LANGUAGES: Language[] = [
  { code: "zh", label: "Chinese", native: "中文", flag: "🇨🇳" },
  { code: "ja", label: "Japanese", native: "日本語", flag: "🇯🇵" },
  { code: "ko", label: "Korean", native: "한국어", flag: "🇰🇷" },
  { code: "vi", label: "Vietnamese", native: "Tiếng Việt", flag: "🇻🇳" },
  { code: "en", label: "English", native: "English", flag: "🇬🇧" },
  { code: "es", label: "Spanish", native: "Español", flag: "🇪🇸" },
  { code: "fr", label: "French", native: "Français", flag: "🇫🇷" },
  { code: "de", label: "German", native: "Deutsch", flag: "🇩🇪" },
  { code: "ru", label: "Russian", native: "Русский", flag: "🇷🇺" },
  { code: "ar", label: "Arabic", native: "العربية", flag: "🇸🇦" },
  { code: "th", label: "Thai", native: "ไทย", flag: "🇹🇭" },
  { code: "hi", label: "Hindi", native: "हिन्दी", flag: "🇮🇳" },
  { code: "pt", label: "Portuguese", native: "Português", flag: "🇵🇹" },
];

export function findLanguage(code: string): Language | undefined {
  return LANGUAGES.find((l) => l.code === code);
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/shared/constants/languages.ts
git commit -m "feat(notes): add shared LANGUAGES constant with flag emojis"
```

---

### Task 3: Simplify EditorContextMenu — remove language submenu

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/EditorContextMenu.tsx`

- [ ] **Step 1: Remove `TRANSLATE_LANGUAGES` array (lines 4–18)**

Delete the entire `const TRANSLATE_LANGUAGES = [...]` block.

- [ ] **Step 2: Remove `onTranslateTo` and `noteTargetLang` from interface and destructuring**

Update `EditorContextMenuProps` — remove these two lines:

```typescript
  onTranslateTo: (
    targetLang: string,
    selectedText?: string,
    rect?: { top: number; left: number },
  ) => void;
```

and:

```typescript
  noteTargetLang?: string;
```

Remove `onTranslateTo` and `noteTargetLang` from the destructuring in the component function signature.

- [ ] **Step 3: Replace translate submenu with a plain MenuItem**

Replace the entire `<ContextMenu.Sub>...</ContextMenu.Sub>` block (lines 116–145) with a plain `MenuItem`. Keep it inside the existing `{hadSelection && (...)}` guard — place it right after the "Create Flashcard" `MenuItem`:

```tsx
              <MenuItem
                onClick={() =>
                  onTranslate(selectionTextRef.current, selectionRectRef.current)
                }
              >
                Translate
              </MenuItem>
```

- [ ] **Step 4: Verify the file compiles**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | grep EditorContextMenu`

Expected: errors in `NoteEditor.tsx` about removed props — fixed in Task 4.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/EditorContextMenu.tsx
git commit -m "feat(notes): simplify context menu — single-click Translate, remove language submenu"
```

---

### Task 4: Clean up NoteEditor — remove `handleTranslateTo` and unsafe casts

**Files:**
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Clean up `usePerspective` destructuring (line 196)**

Change:

```typescript
  const { setLanguagePair, languagePair } = usePerspective(
    note.id,
    editor,
    (note as Record<string, unknown>).perspectiveConfig as string | null | undefined,
  );
```

To:

```typescript
  usePerspective(note.id, editor, note.perspectiveConfig);
```

`setLanguagePair` and `languagePair` are no longer used in `NoteEditor` — the panel handles language selection now. The call is kept without assignment because the hook still registers the cursor-tracking `useEffect` side effect. No variable assignment avoids unused-variable lint warnings.

- [ ] **Step 2: Fix `useLanguageConfig` call — remove unsafe cast (lines 203–206)**

Change:

```typescript
  const { sourceLang, targetLang } = useLanguageConfig(
    (note as Record<string, unknown>).perspectiveConfig as string | null | undefined,
    note.body ?? undefined,
  );
```

To:

```typescript
  const { sourceLang, targetLang } = useLanguageConfig(
    note.perspectiveConfig,
    note.body ?? undefined,
  );
```

- [ ] **Step 3: Remove `handleTranslateTo` (lines 236–244)**

Delete the entire `handleTranslateTo` callback.

- [ ] **Step 4: Remove `onTranslateTo` and `noteTargetLang` from EditorContextMenu props (lines 473–476)**

Change the `EditorContextMenu` JSX from:

```tsx
              <EditorContextMenu
                onAnnotate={handleAnnotate}
                onFlashcard={handleFlashcard}
                onTranslate={handleTranslate}
                onTranslateTo={handleTranslateTo}
                onAskAI={handleAskAI}
                onRemoveAnnotation={handleRemoveAnnotation}
                noteTargetLang={languagePair?.targetLang}
              >
```

To:

```tsx
              <EditorContextMenu
                onAnnotate={handleAnnotate}
                onFlashcard={handleFlashcard}
                onTranslate={handleTranslate}
                onAskAI={handleAskAI}
                onRemoveAnnotation={handleRemoveAnnotation}
              >
```

- [ ] **Step 5: Verify compilation**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | head -20`

Expected: clean (or only errors from AISuggestionsPanel not yet updated).

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/components/NoteEditor.tsx
git commit -m "feat(notes): remove handleTranslateTo and unsafe perspectiveConfig casts"
```

---

### Task 5: Add Language section to AISuggestionsPanel

**Files:**
- Modify: `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx`

This is the core UI change. The Language section sits **above** the existing collapsible "AI Suggestions" section, always visible.

- [ ] **Step 1: Add imports**

Add to imports:

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { LANGUAGES, findLanguage } from "@shared/constants/languages";
import { useLanguageConfig } from "../hooks/useLanguageConfig";
```

- [ ] **Step 2: Add `perspectiveConfig` prop**

Update the interface:

```typescript
interface AISuggestionsPanelProps {
  noteId: string | null;
  perspectiveConfig: string | null;
  onSelectNote: (id: string) => void;
  onOpenInsight?: () => void;
}
```

Add `perspectiveConfig` to the destructuring.

- [ ] **Step 3: Add language config hook and save handler inside the component**

After the existing `useNoteSuggestions` call, add:

```typescript
  const { sourceLang, targetLang } = useLanguageConfig(perspectiveConfig);

  const handleLanguageChange = useCallback(
    (field: "sourceLang" | "targetLang", code: string) => {
      if (!noteId) return;
      // Parse existing config, merge new language pair, save
      let config: Record<string, unknown> = {};
      if (perspectiveConfig) {
        try {
          config = JSON.parse(perspectiveConfig);
        } catch {
          // ignore
        }
      }
      const pair = (config.languagePair as Record<string, string>) ?? {};
      config.languagePair = { ...pair, [field]: code };
      ipc("note_update", {
        params: { id: noteId, perspectiveConfig: JSON.stringify(config) },
      }).catch(() => {});
    },
    [noteId, perspectiveConfig],
  );
```

Update React import to include `useCallback` and `useRef`:

```typescript
import { useCallback, useEffect, useRef, useState } from "react";
```

Add `createPortal` import (for the portal-rendered dropdown):

```typescript
import { createPortal } from "react-dom";
```

- [ ] **Step 4: Add Language section JSX**

Insert **before** the existing `<div className="border-b border-border" ...>` (the AI Suggestions collapsible), wrap the whole return in a fragment and add the Language section first:

```tsx
  return (
    <>
      {/* Language config — always visible */}
      <div className="border-b border-border px-3 py-2">
        <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1.5">
          Language
        </div>
        <div className="flex gap-2">
          <LanguageDropdown
            label="Source"
            value={sourceLang}
            onChange={(code) => handleLanguageChange("sourceLang", code)}
            disabled={!noteId}
          />
          <LanguageDropdown
            label="Target"
            value={targetLang}
            onChange={(code) => handleLanguageChange("targetLang", code)}
            disabled={!noteId}
          />
        </div>
      </div>

      {/* Existing AI Suggestions section — move entire existing return body here */}
      <div className="border-b border-border" style={{ borderLeftColor: ACCENT, borderLeftWidth: 2 }}>
        {/* The entire existing collapsible section (button + content) stays here unchanged.
            Move lines 47–203 of the current file into this slot. */}
      </div>
    </>
  );
```

**Important:** The existing `return (...)` expression (the single `<div>` with the collapsible AI Suggestions content) becomes the second child of the fragment. Wrap it — don't duplicate it.

- [ ] **Step 5: Add `LanguageDropdown` component**

Add this private component at the bottom of the file (before the closing):

```tsx
function LanguageDropdown({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (code: string) => void;
  disabled: boolean;
}) {
  const [open, setOpen] = useState(false);
  const lang = findLanguage(value);
  const display = lang ? `${lang.flag} ${lang.native}` : value;

  return (
    <div className="flex-1 relative">
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between gap-1 px-2 py-1 rounded-md text-[11px] text-muted-foreground bg-surface-base hover:bg-surface-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <span className="truncate">{display}</span>
        <ChevronDown size={10} className="shrink-0 text-dim" />
      </button>
      <div className="text-[9px] text-dim mt-0.5 px-1">{label}</div>
      {open &&
        createPortal(
          <div
            className="fixed glass-panel rounded-lg py-1 shadow-xl z-[100] max-h-[240px] overflow-y-auto"
            style={{
              top: ref.current ? ref.current.getBoundingClientRect().bottom + 4 : 0,
              left: ref.current ? ref.current.getBoundingClientRect().left : 0,
              width: ref.current ? ref.current.getBoundingClientRect().width : "auto",
            }}
          >
            {LANGUAGES.map((lang) => (
              <button
                key={lang.code}
                type="button"
                onClick={() => {
                  onChange(lang.code);
                  setOpen(false);
                }}
                className={`w-full text-left px-2 py-1 text-[11px] transition-colors ${
                  lang.code === value
                    ? "text-foreground bg-accent"
                    : "text-muted-foreground hover:bg-surface-hover hover:text-foreground"
                }`}
              >
                {lang.flag} {lang.native}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}
```

- [ ] **Step 6: Add click-outside handler for dropdown**

Inside `LanguageDropdown`, add after the `open` state:

```typescript
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);
```

And wrap the dropdown's root `<div>` with `ref={ref}`.

`useRef` and `useEffect` are already imported in Step 3's updated React import line.

- [ ] **Step 7: Verify compilation**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | head -20`

Expected: error in `ContextPanel.tsx` about missing `perspectiveConfig` prop — fixed in Task 6.

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx
git commit -m "feat(notes): add Language section with source/target dropdowns to AI Suggestions panel"
```

---

### Task 6: Wire ContextPanel — pass `perspectiveConfig` to AISuggestionsPanel

**Files:**
- Modify: `desktop-ui/src/features/notes/components/ContextPanel.tsx:251-255`

- [ ] **Step 1: Add `perspectiveConfig` prop to AISuggestionsPanel call site**

Change (line 251–255):

```tsx
      <AISuggestionsPanel
        noteId={noteId}
        onSelectNote={onSelectNote}
        onOpenInsight={onOpenInsight}
      />
```

To:

```tsx
      <AISuggestionsPanel
        noteId={noteId}
        perspectiveConfig={note.perspectiveConfig ?? null}
        onSelectNote={onSelectNote}
        onOpenInsight={onOpenInsight}
      />
```

No changes to `ContextPanelProps` — `note` already has `perspectiveConfig` after Task 1.

- [ ] **Step 2: Verify full compilation**

Run: `cd desktop-ui && bunx tsc --noEmit`

Expected: clean, no errors.

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint`

Expected: clean (warnings are OK per Biome config).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/ContextPanel.tsx
git commit -m "feat(notes): wire perspectiveConfig from ContextPanel to AISuggestionsPanel"
```

---

### Task 7: Final verification

- [ ] **Step 1: Run full lint with auto-fix**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 2: Start dev server and verify visually**

Run: `cd desktop-ui && bun run dev` (in background)

Then: `cargo tauri dev`

Verify:
1. Open a note → Language section appears at top of right panel with two dropdowns
2. Change target language via dropdown → selection persists when reopening the note
3. Select text → right-click → "Translate" is a single menu item (no submenu)
4. Click "Translate" → QuickTranslatePopup appears with the correct language pair
5. The dropdown shows flag + native name for each language

- [ ] **Step 3: Final commit if lint:fix made changes**

```bash
git add -u && git commit -m "style: lint fixes"
```
