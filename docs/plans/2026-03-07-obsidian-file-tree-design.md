# Obsidian-Style File Tree Sidebar

**Date:** 2026-03-07
**Status:** Approved

## Summary

Replace the current 3-column notes layout (NotebookTree + NoteList + Editor) with a 2-column Obsidian-style layout (FileTree + Editor). The unified FileTree shows folders and notes in a single collapsible tree hierarchy.

## Layout

```
Sidebar | FileTree (w-64) | Editor (flex-1)
```

## FileTree Component

**Header:**
- Search bar at top
- New note + New folder buttons

**Tree:**
- Folders (notebooks) render with chevron expand/collapse
- Notes render as leaf items with file icon
- Sort: folders first (alphabetical), then notes (alphabetical)
- Root-level notes (notebookId=null) appear after all root folders
- Notes show title + subtle last-modified date
- Vertical indent guides for nesting depth

**Context menus (right-click):**
- Folder: New note here, New subfolder, Rename, Delete
- Note: Pin/Unpin, Delete

**Interactions:**
- Click note → open in editor
- Click folder → expand/collapse
- Cmd+N → new note (in selected folder or root)

## Removed

- `NoteList` component
- `NoteCard` component (note items are inline tree nodes now)
- `showListPanel` toggle
- "All Notes" button
- Tag filter chips in sidebar

## Data Model

No backend changes needed. Existing types suffice:
- `Notebook.parentId` → folder nesting
- `Note.notebookId` → note-to-folder relationship
- `Note.notebookId = null` → root-level note
