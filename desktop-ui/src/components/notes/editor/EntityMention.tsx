import { Mark, mergeAttributes } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ipc } from "../../../hooks/useIpc";
import type { Project, Task } from "../../../lib/types";

// ── EntityMention Mark ──────────────────────────────────────────────────

export interface EntityMentionOptions {
  onNavigate: (entityType: string, entityId: string) => void;
}

export const EntityMentionMark = Mark.create<EntityMentionOptions>({
  name: "entityMention",
  inclusive: false,
  excludes: "link wikiLink",

  addOptions() {
    return { onNavigate: () => {} };
  },

  addAttributes() {
    return {
      entityType: { default: null },
      entityId: { default: null },
    };
  },

  parseHTML() {
    return [
      {
        tag: "span[data-entity-mention]",
        getAttrs: (el) => ({
          entityType: (el as HTMLElement).getAttribute("data-entity-type") || null,
          entityId: (el as HTMLElement).getAttribute("data-entity-id") || null,
        }),
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const entityType = HTMLAttributes.entityType || "task";
    return [
      "span",
      mergeAttributes(HTMLAttributes, {
        "data-entity-mention": "",
        "data-entity-type": entityType,
        "data-entity-id": HTMLAttributes.entityId || "",
        class: `entity-mention entity-mention--${entityType}`,
      }),
      0,
    ];
  },

  addProseMirrorPlugins() {
    const onNavigate = this.options.onNavigate;
    return [
      new Plugin({
        key: new PluginKey("entityMentionClick"),
        props: {
          handleDOMEvents: {
            click: (_view, event) => {
              const target = event.target as HTMLElement;
              const el = target.closest?.("span[data-entity-mention]") as HTMLElement | null;
              if (el) {
                event.preventDefault();
                const entityType = el.getAttribute("data-entity-type");
                const entityId = el.getAttribute("data-entity-id");
                if (entityType && entityId) onNavigate(entityType, entityId);
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

// ── EntityMention Autocomplete Extension ────────────────────────────────

type MentionMenuState = {
  from: number;
  query: string;
} | null;

// Module-level callback bridge: ProseMirror plugins run outside React's
// lifecycle, so we use a module-scoped callback to notify the React
// autocomplete menu component of state changes (position, query text).
let mentionMenuCallback:
  | ((state: MentionMenuState, coords: { x: number; y: number }) => void)
  | null = null;

export function setMentionMenuCallback(
  cb: ((state: MentionMenuState, coords: { x: number; y: number }) => void) | null,
) {
  mentionMenuCallback = cb;
}

let mentionMenuFrom: number | null = null;

export function resetMentionMenu() {
  mentionMenuFrom = null;
  mentionMenuCallback?.(null, { x: 0, y: 0 });
}

export const EntityMentionAutocomplete = Mark.create({
  name: "entityMentionAutocomplete",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("entityMentionAutocomplete"),
        props: {
          handleTextInput: (view, from, _to, text) => {
            if (text !== "@") return false;

            // Only trigger at start of line or after whitespace
            const charBefore = from > 0 ? view.state.doc.textBetween(from - 1, from) : "";
            if (charBefore.length > 0 && !/\s/.test(charBefore)) return false;

            mentionMenuFrom = from;
            // Schedule after the "@" is inserted
            setTimeout(() => {
              if (mentionMenuFrom === null) return;
              const coords = view.coordsAtPos(from);
              mentionMenuCallback?.(
                { from: mentionMenuFrom, query: "" },
                { x: coords.left, y: coords.bottom },
              );
            }, 0);
            return false;
          },

          handleKeyDown: (view, event) => {
            if (mentionMenuFrom === null) return false;

            if (event.key === "Escape" || event.key === " ") {
              resetMentionMenu();
              return false;
            }

            // Update query on typing
            if (event.key.length === 1 || event.key === "Backspace") {
              setTimeout(() => {
                if (mentionMenuFrom === null) return;
                const state = view.state;
                const cursorPos = state.selection.from;
                if (cursorPos <= mentionMenuFrom) {
                  resetMentionMenu();
                  return;
                }
                const query = state.doc.textBetween(mentionMenuFrom + 1, cursorPos);
                const coords = view.coordsAtPos(cursorPos);
                mentionMenuCallback?.(
                  { from: mentionMenuFrom, query },
                  { x: coords.left, y: coords.bottom },
                );
              }, 0);
            }

            return false;
          },
        },
      }),
    ];
  },
});

// ── EntityMention Autocomplete Menu (React) ─────────────────────────────

interface MentionResult {
  entityType: "task" | "project";
  entityId: string;
  title: string;
}

interface EntityMentionMenuProps {
  editor: ReturnType<typeof import("@tiptap/react").useEditor>;
}

export function EntityMentionMenu({ editor }: EntityMentionMenuProps) {
  const [state, setState] = useState<MentionMenuState>(null);
  const [coords, setCoords] = useState({ x: 0, y: 0 });
  const [results, setResults] = useState<MentionResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const selectedIndexRef = useRef(0);
  selectedIndexRef.current = selectedIndex;
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setMentionMenuCallback((s, c) => {
      setState(s);
      setCoords(c);
      if (!s) {
        setResults([]);
        setSelectedIndex(0);
      }
    });
    return () => {
      resetMentionMenu();
      setMentionMenuCallback(null);
    };
  }, []);

  // Fetch task/project lists once when the menu opens, filter locally on keystroke
  const cacheRef = useRef<{ tasks: Task[]; projects: Project[] } | null>(null);
  const isOpen = state !== null;
  const query = state?.query ?? "";

  useEffect(() => {
    if (!isOpen) {
      cacheRef.current = null;
      setResults([]);
      setSelectedIndex(0);
      return;
    }

    let cancelled = false;

    const run = async () => {
      // Fetch once per open, reuse cache for subsequent keystrokes
      if (!cacheRef.current) {
        try {
          const [tasks, projects] = await Promise.all([
            ipc<Task[]>("task_list", undefined),
            ipc<Project[]>("project_list", undefined),
          ]);
          if (cancelled) return;
          cacheRef.current = { tasks, projects };
        } catch (e) {
          console.warn("Entity mention search failed:", e);
          if (!cancelled) setResults([]);
          return;
        }
      }
      if (cancelled) return;

      const { tasks, projects } = cacheRef.current;
      const lower = query.toLowerCase();
      const matched: MentionResult[] = [];
      for (const t of tasks) {
        if (matched.length >= 5) break;
        if (!lower || t.title.toLowerCase().includes(lower)) {
          matched.push({ entityType: "task", entityId: t.id, title: t.title });
        }
      }
      const taskCount = matched.length;
      for (const p of projects) {
        if (matched.length - taskCount >= 3) break;
        if (!lower || p.name.toLowerCase().includes(lower)) {
          matched.push({ entityType: "project", entityId: p.id, title: p.name });
        }
      }
      setResults(matched);
      setSelectedIndex(0);
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [isOpen, query]);

  const insertMention = useCallback(
    (item: MentionResult) => {
      if (!editor || !state) return;
      const cursorPos = editor.state.selection.from;
      editor
        .chain()
        .focus()
        .deleteRange({ from: state.from, to: cursorPos })
        .insertContent({
          type: "text",
          text: `@${item.title}`,
          marks: [
            {
              type: "entityMention",
              attrs: { entityType: item.entityType, entityId: item.entityId },
            },
          ],
        })
        .insertContent(" ")
        .run();
      resetMentionMenu();
      setState(null);
    },
    [editor, state],
  );

  // Keyboard navigation
  const resultsRef = useRef<MentionResult[]>([]);
  resultsRef.current = results;
  const insertRef = useRef(insertMention);
  insertRef.current = insertMention;

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
      className="fixed z-50 glass-dropdown rounded-xl py-1 min-w-[220px] max-w-[320px]"
      style={{ left: coords.x, top: coords.y + 4 }}
    >
      {results.length === 0 && (
        <div className="px-3 py-2 text-xs text-dim">
          {state.query ? "No matching entities" : "Type to search tasks & projects..."}
        </div>
      )}
      {results.map((item, i) => (
        <button
          key={`${item.entityType}-${item.entityId}`}
          type="button"
          onClick={() => insertMention(item)}
          className={`w-full px-3 py-1.5 text-sm text-left flex items-center gap-2 transition-colors ${
            i === selectedIndex
              ? "bg-white/[0.08] text-primary"
              : "text-secondary hover:bg-white/[0.04]"
          }`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
              item.entityType === "task" ? "bg-brand" : "bg-info"
            }`}
          />
          <span className="truncate">{item.title}</span>
          <span className="text-[10px] text-dim ml-auto shrink-0">{item.entityType}</span>
        </button>
      ))}
    </div>,
    document.body,
  );
}
