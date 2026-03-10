import { ipc } from "@shared/hooks/useIpc";
import type { Note } from "@shared/types";
import { Mark, mergeAttributes } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

// ── WikiLink Mark ────────────────────────────────────────────────────────

export interface WikiLinkOptions {
  onNavigate: (noteId: string) => void;
}

export const WikiLinkMark = Mark.create<WikiLinkOptions>({
  name: "wikiLink",
  inclusive: false,
  excludes: "link",

  addOptions() {
    return { onNavigate: () => {} };
  },

  addAttributes() {
    return {
      noteId: { default: null },
      title: { default: null },
    };
  },

  parseHTML() {
    return [
      {
        tag: "span[data-wiki-link]",
        getAttrs: (el) => ({
          noteId: (el as HTMLElement).getAttribute("data-note-id") || null,
          title: (el as HTMLElement).getAttribute("data-title") || null,
        }),
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(HTMLAttributes, {
        "data-wiki-link": "",
        "data-note-id": HTMLAttributes.noteId || "",
        "data-title": HTMLAttributes.title || "",
        class: "wiki-link",
      }),
      0,
    ];
  },

  addProseMirrorPlugins() {
    const onNavigate = this.options.onNavigate;
    return [
      new Plugin({
        key: new PluginKey("wikiLinkClick"),
        props: {
          handleDOMEvents: {
            click: (_view, event) => {
              const target = event.target as HTMLElement;
              const el = target.closest?.("span[data-wiki-link]") as HTMLElement | null;
              if (el) {
                event.preventDefault();
                const noteId = el.getAttribute("data-note-id");
                if (noteId) onNavigate(noteId);
                return true;
              }
              return false;
            },
          },
        },
      }),
    ];
  },
});

// ── WikiLink Autocomplete Extension ──────────────────────────────────────

type WikiLinkMenuState = {
  from: number;
  query: string;
} | null;

// Module-level callback bridge: ProseMirror plugins run outside React's
// lifecycle, so we use a module-scoped callback to notify the React
// autocomplete menu component of state changes (position, query text).
let wikiLinkMenuCallback:
  | ((state: WikiLinkMenuState, coords: { x: number; y: number }) => void)
  | null = null;

export function setWikiLinkMenuCallback(
  cb: ((state: WikiLinkMenuState, coords: { x: number; y: number }) => void) | null,
) {
  wikiLinkMenuCallback = cb;
}

let wikiLinkMenuFrom: number | null = null;

export function resetWikiLinkMenu() {
  wikiLinkMenuFrom = null;
  wikiLinkMenuCallback?.(null, { x: 0, y: 0 });
}

export const WikiLinkAutocomplete = Mark.create({
  name: "wikiLinkAutocomplete",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("wikiLinkAutocomplete"),
        props: {
          handleTextInput: (view, from, _to, text) => {
            if (text === "[") {
              const charBefore = from > 0 ? view.state.doc.textBetween(from - 1, from) : "";
              if (charBefore === "[") {
                wikiLinkMenuFrom = from - 1;
                const coords = view.coordsAtPos(from);
                wikiLinkMenuCallback?.(
                  { from: wikiLinkMenuFrom, query: "" },
                  { x: coords.left, y: coords.bottom },
                );
                return false;
              }
            }

            if (text === "]" && wikiLinkMenuFrom !== null) {
              resetWikiLinkMenu();
              return false;
            }

            if (wikiLinkMenuFrom !== null) {
              setTimeout(() => {
                const state = view.state;
                const cursorPos = state.selection.from;
                if (wikiLinkMenuFrom !== null && cursorPos > wikiLinkMenuFrom + 2) {
                  const query = state.doc.textBetween(wikiLinkMenuFrom + 2, cursorPos);
                  const coords = view.coordsAtPos(cursorPos);
                  wikiLinkMenuCallback?.(
                    { from: wikiLinkMenuFrom, query },
                    { x: coords.left, y: coords.bottom },
                  );
                }
              }, 0);
            }

            return false;
          },
        },
      }),
    ];
  },
});

// ── WikiLink Autocomplete Menu (React) ──────────────────────────────────

interface WikiLinkMenuProps {
  editor: ReturnType<typeof import("@tiptap/react").useEditor>;
}

export function WikiLinkMenu({ editor }: WikiLinkMenuProps) {
  const [state, setState] = useState<WikiLinkMenuState>(null);
  const [coords, setCoords] = useState({ x: 0, y: 0 });
  const [results, setResults] = useState<Note[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const selectedIndexRef = useRef(0);
  selectedIndexRef.current = selectedIndex;
  const menuRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setWikiLinkMenuCallback((s, c) => {
      setState(s);
      setCoords(c);
      if (!s) {
        setResults([]);
        setSelectedIndex(0);
      }
    });
    return () => {
      resetWikiLinkMenu();
      setWikiLinkMenuCallback(null);
    };
  }, []);

  // Search notes as user types
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!state?.query) {
      setResults([]);
      setSelectedIndex(0);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      try {
        const notes = await ipc<Note[]>("note_search", { query: state.query });
        setResults(notes.slice(0, 8));
        setSelectedIndex(0);
      } catch (e) {
        console.warn("Wiki-link search failed:", e);
        setResults([]);
      }
    }, 150);
  }, [state?.query]);

  const insertWikiLink = useCallback(
    (note: Note) => {
      if (!editor || !state) return;
      const cursorPos = editor.state.selection.from;
      editor
        .chain()
        .focus()
        .deleteRange({ from: state.from, to: cursorPos })
        .insertContent({
          type: "text",
          text: note.title,
          marks: [{ type: "wikiLink", attrs: { noteId: note.id, title: note.title } }],
        })
        .run();
      resetWikiLinkMenu();
      setState(null);
    },
    [editor, state],
  );

  // Keyboard navigation — use refs to keep the listener stable
  const resultsRef = useRef<Note[]>([]);
  resultsRef.current = results;
  const insertRef = useRef(insertWikiLink);
  insertRef.current = insertWikiLink;

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      const r = resultsRef.current;
      if (r.length === 0) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => (i + 1) % r.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => (i - 1 + r.length) % r.length);
      } else if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        insertRef.current(r[selectedIndexRef.current]);
      } else if (e.key === "Escape") {
        e.preventDefault();
        setState(null);
      }
    },
    [], // stable — reads from refs
  );

  useEffect(() => {
    if (!state) return;
    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, [state, handleKeyDown]);

  if (!state) return null;

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-50 glass-dropdown rounded-xl py-1 min-w-[200px] max-w-[300px]"
      style={{ left: coords.x, top: coords.y + 4 }}
    >
      {results.length === 0 && (
        <div className="px-3 py-2 text-xs text-dim">
          {state.query ? "No matching notes" : "Type to search notes..."}
        </div>
      )}
      {results.map((note, i) => (
        <button
          key={note.id}
          type="button"
          onClick={() => insertWikiLink(note)}
          className={`w-full px-3 py-1.5 text-sm text-left flex items-center gap-2 transition-colors ${
            i === selectedIndex
              ? "bg-white/[0.08] text-primary"
              : "text-secondary hover:bg-white/[0.04]"
          }`}
        >
          <span className="truncate">{note.title}</span>
        </button>
      ))}
    </div>,
    document.body,
  );
}
