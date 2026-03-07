# Vim Mode for Notes Editor — Design

## Overview

Add comprehensive vim keybindings to the TipTap-based notes editor, toggled via a button in the formatting toolbar. When active, the toolbar collapses to a minimal mode indicator (`-- NORMAL --`) and all editing happens through vim grammar.

**Scope:** Comprehensive vim (motions, operators, text objects, visual mode, dot repeat, search, marks, registers, counts). Not full Neovim (no macros, no ex commands beyond `:w`, no window splits).

**Approach:** Custom ProseMirror plugin with a pure vim state machine. No external vim library — the state machine translates vim grammar into ProseMirror transactions, leveraging TipTap's existing commands for formatting.

## Architecture

### State Machine (Pure)

`VimState` is the core — a pure function: `(keystroke, editorState) -> VimAction`.

```
VimState {
  mode:          Normal | Insert | Visual | VisualLine
  count:         number | null        — prefix count buffer (3dw)
  operator:      d | c | y | > | < | gu | gU | null
  awaitingChar:  f | F | t | T | r | null
  registers:     Map<string, string>  — "a"-"z" + "" (default)
  marks:         Map<string, number>  — mark name -> doc position
  lastAction:    RecordedAction       — for dot repeat
  searchPattern: string | null
  searchDir:     forward | backward
  visualAnchor:  number | null        — doc pos where v/V started
}
```

### Keystroke Processing (Normal Mode)

```
digit?       -> accumulate into count
operator?    -> set pending, await motion/text-object
motion?      -> if operator: apply over range; else: move cursor
text-object? -> only after operator; apply operator over range
command?     -> execute directly (x, dd, p, u, i, ...)
```

### Motions

| Category   | Keys                          | Behavior                              |
|------------|-------------------------------|---------------------------------------|
| Character  | h l                           | left/right within line                |
| Line       | j k                           | up/down (preserve column)             |
| Word       | w W b B e E                   | word boundaries (small/WORD)          |
| Line pos   | 0 $ ^                         | line start / end / first non-ws       |
| Document   | gg G                          | doc start / end (or line N)           |
| Find char  | f{c} F{c} t{c} T{c}          | find char forward/back, to/till       |
| Paragraph  | { }                           | prev/next blank line                  |
| Match      | %                             | matching bracket                      |
| Search     | n N                           | next/prev search match                |

### Text Objects (after operator or in visual)

| Keys       | Inner              | Around                  |
|------------|--------------------|-------------------------|
| w          | word chars          | word + surrounding space |
| s          | sentence            | sentence + trailing space|
| p          | paragraph           | paragraph + blank lines  |
| " ' `      | inside quotes       | including quotes         |
| ( ) b      | inside parens       | including parens         |
| [ ]        | inside brackets     | including brackets       |
| { } B      | inside braces       | including braces         |

### Operators

| Key   | Action         | Notes                              |
|-------|----------------|------------------------------------|
| d     | Delete         | Yanks to register before deleting  |
| c     | Change         | Delete + enter Insert              |
| y     | Yank           | Copy to register                   |
| > <   | Indent/outdent | TipTap list sink/lift              |
| gu gU | Case transform | Lower/upper over range             |

### Commands (no operator)

| Key              | Action                                        |
|------------------|-----------------------------------------------|
| i a o O I A      | Enter Insert (various positions)              |
| x X              | Delete char forward/backward                  |
| dd yy cc         | Doubled operator = whole line                 |
| p P              | Paste after/before                            |
| u Ctrl+R         | Undo/redo (ProseMirror history)               |
| J                | Join line with next                           |
| .                | Repeat last change                            |
| v V              | Visual / Visual Line mode                     |
| r{c}             | Replace char under cursor                     |
| ~                | Toggle case of char                           |
| / :w             | Open command line                             |

## UI

### Toolbar Behavior

- **Vim OFF:** Full formatting toolbar as-is. `Vi` toggle button appended at far right, separated by a divider.
- **Vim ON:** Toolbar collapses to `-- NORMAL --` text (left, monochrome, mono font) + `Vi` toggle button (right, active/brand color).

### Mode Indicator

Classic vim style, monochrome:
- `-- NORMAL --`
- `-- INSERT --`
- `-- VISUAL --`
- `-- VISUAL LINE --`

Rendered in `font-mono text-xs text-secondary`.

### Command Line

Thin input at bottom of editor area. Appears on `/` or `:` in Normal mode. Shows pattern/command as typed. Enter confirms, Escape cancels. Search highlights matches with decorations.

### Cursor

- **Normal:** Block cursor (CSS overlay — `caret-color: transparent` + positioned span over current char)
- **Insert:** Standard thin caret
- **Visual:** Selection highlight + block cursor at selection head

### Persistence

Vim mode on/off saved to `localStorage("klyntbot:notes:vimMode")`.

## File Structure

```
desktop-ui/src/components/notes/editor/
  vim/
    VimState.ts          -- Pure state machine
    VimPlugin.ts         -- ProseMirror plugin (handleKeyDown -> dispatch)
    motions.ts           -- Cursor position calculations
    textObjects.ts       -- Range calculations for text objects
    operators.ts         -- Delete, yank, change, indent, case
    commands.ts          -- Insert entries, paste, join, repeat
    search.ts            -- Pattern matching + highlight decorations
    cursor.ts            -- Block cursor decoration plugin
    index.ts             -- TipTap VimMode extension export
  VimStatusLine.tsx      -- Mode indicator component
  VimCommandLine.tsx     -- Bottom input for / and :
  EditorToolbar.tsx      -- Modified: conditional vim/full rendering
  EditorCore.tsx         -- Modified: VimMode extension registration
  NoteEditor.tsx         -- Modified: vim toggle + status/command line
```

## Key Implementation Notes

- Motions operate on flattened text content. ProseMirror's `TextSelection` handles cross-node positions natively, so `w` jumping across paragraph boundaries works naturally.
- The state machine is pure and independently testable — no ProseMirror dependency in VimState.ts.
- Undo/redo delegates to ProseMirror's built-in history plugin.
- Dot repeat records the sequence of keys that produced the last change, then replays them through the state machine.
- Visual mode uses ProseMirror's `TextSelection` (with anchor at visualAnchor), Visual Line extends to full line boundaries.
