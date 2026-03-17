import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { Editor, Range } from "@tiptap/react";
import { Extension } from "@tiptap/react";
import {
  Code,
  Heading1,
  Heading2,
  Heading3,
  List,
  ListChecks,
  ListOrdered,
  Minus,
  Pilcrow,
  Quote,
  Sigma,
  Table,
} from "lucide-react";
import { type ComponentType, useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

// ── Slash command definitions ──────────────────────────────────

interface SlashCommand {
  label: string;
  aliases: string[];
  icon: ComponentType<{ className?: string }>;
  action: (editor: Editor, range: Range) => void;
}

const slashCommands: SlashCommand[] = [
  {
    label: "Heading 1",
    aliases: ["h1", "heading1", "title"],
    icon: Heading1,
    action: (e, r) => e.chain().focus().deleteRange(r).setHeading({ level: 1 }).run(),
  },
  {
    label: "Heading 2",
    aliases: ["h2", "heading2", "subtitle"],
    icon: Heading2,
    action: (e, r) => e.chain().focus().deleteRange(r).setHeading({ level: 2 }).run(),
  },
  {
    label: "Heading 3",
    aliases: ["h3", "heading3"],
    icon: Heading3,
    action: (e, r) => e.chain().focus().deleteRange(r).setHeading({ level: 3 }).run(),
  },
  {
    label: "Paragraph",
    aliases: ["p", "paragraph", "text"],
    icon: Pilcrow,
    action: (e, r) => e.chain().focus().deleteRange(r).setParagraph().run(),
  },
  {
    label: "Bullet List",
    aliases: ["ul", "list", "bullet"],
    icon: List,
    action: (e, r) => e.chain().focus().deleteRange(r).toggleBulletList().run(),
  },
  {
    label: "Ordered List",
    aliases: ["ol", "numbered", "ordered"],
    icon: ListOrdered,
    action: (e, r) => e.chain().focus().deleteRange(r).toggleOrderedList().run(),
  },
  {
    label: "Task List",
    aliases: ["task", "tasks", "todo", "checklist"],
    icon: ListChecks,
    action: (e, r) => e.chain().focus().deleteRange(r).toggleTaskList().run(),
  },
  {
    label: "Code Block",
    aliases: ["cb", "codeblock", "code"],
    icon: Code,
    action: (e, r) => e.chain().focus().deleteRange(r).toggleCodeBlock().run(),
  },
  {
    label: "Blockquote",
    aliases: ["quote", "blockquote"],
    icon: Quote,
    action: (e, r) => e.chain().focus().deleteRange(r).toggleBlockquote().run(),
  },
  {
    label: "Table",
    aliases: ["table", "grid"],
    icon: Table,
    action: (e, r) =>
      e.chain().focus().deleteRange(r).insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(),
  },
  {
    label: "Horizontal Rule",
    aliases: ["hr", "divider", "line", "separator"],
    icon: Minus,
    action: (e, r) => e.chain().focus().deleteRange(r).setHorizontalRule().run(),
  },
  {
    label: "Math Block",
    aliases: ["math", "equation", "latex", "katex"],
    icon: Sigma,
    action: (e, r) => {
      e.chain()
        .focus()
        .deleteRange(r)
        .insertContent({ type: "mathBlock", attrs: { tex: "E = mc^2" } })
        .run();
    },
  },
];

// ── Slash menu state (shared between extension and component) ──

interface SlashMenuState {
  active: boolean;
  query: string;
  range: Range;
  x: number;
  y: number;
}

type SlashMenuCallback = (state: SlashMenuState | null) => void;

let slashMenuCallback: SlashMenuCallback | null = null;

export function setSlashMenuCallback(cb: SlashMenuCallback | null) {
  slashMenuCallback = cb;
}

// ── TipTap Extension ───────────────────────────────────────────

const slashPluginKey = new PluginKey("slashCommands");

export const SlashCommandsExtension = Extension.create({
  name: "slashCommands",

  addProseMirrorPlugins() {
    const editor = this.editor;
    return [
      new Plugin({
        key: slashPluginKey,
        props: {
          handleTextInput(_view, from, _to, text) {
            if (text !== "/") return false;

            // Only trigger at start of line or after whitespace
            const { state } = editor;
            const $pos = state.doc.resolve(from);
            const textBefore = $pos.parent.textBetween(0, $pos.parentOffset, undefined, "\ufffc");

            if (textBefore.length > 0 && !/\s$/.test(textBefore)) return false;

            // Schedule menu open after the "/" is inserted
            requestAnimationFrame(() => {
              const coords = editor.view.coordsAtPos(from);
              slashMenuCallback?.({
                active: true,
                query: "",
                range: { from, to: from + 1 },
                x: coords.left,
                y: coords.bottom + 4,
              });
            });

            return false;
          },

          handleKeyDown() {
            // Let the React component handle navigation when menu is open
            return false;
          },
        },

        view() {
          return {
            update(view) {
              // Track query changes while menu is open
              const { state } = view;
              const { selection } = state;
              const { $anchor } = selection;

              if (!slashMenuCallback) return;

              const textBefore = $anchor.parent.textBetween(
                0,
                $anchor.parentOffset,
                undefined,
                "\ufffc",
              );
              const match = textBefore.match(/(^|\s)\/([^\s]*)$/);

              if (match) {
                const from = $anchor.pos - match[2].length - 1;
                const to = $anchor.pos;
                const coords = view.coordsAtPos(from);
                slashMenuCallback({
                  active: true,
                  query: match[2],
                  range: { from, to },
                  x: coords.left,
                  y: coords.bottom + 4,
                });
              } else {
                slashMenuCallback(null);
              }
            },
          };
        },
      }),
    ];
  },
});

// ── React Component ────────────────────────────────────────────

interface SlashMenuProps {
  editor: Editor;
}

export function SlashMenu({ editor }: SlashMenuProps) {
  const [state, setState] = useState<SlashMenuState | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setSlashMenuCallback((newState) => {
      setState(newState);
      if (newState) setSelectedIndex(0);
    });
    return () => setSlashMenuCallback(null);
  }, []);

  const filtered = state
    ? slashCommands.filter((cmd) => {
        if (!state.query) return true;
        const q = state.query.toLowerCase();
        return cmd.label.toLowerCase().includes(q) || cmd.aliases.some((a) => a.includes(q));
      })
    : [];

  const executeCommand = useCallback(
    (index: number) => {
      if (!state || !filtered[index]) return;
      filtered[index].action(editor, state.range);
      setState(null);
    },
    [editor, state, filtered],
  );

  // Keyboard handler
  useEffect(() => {
    if (!state) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => (i + 1) % Math.max(filtered.length, 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex(
          (i) => (i - 1 + Math.max(filtered.length, 1)) % Math.max(filtered.length, 1),
        );
      } else if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        executeCommand(selectedIndex);
      } else if (e.key === "Escape") {
        e.preventDefault();
        setState(null);
      }
    };

    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, [state, selectedIndex, filtered.length, executeCommand]);

  // Scroll selected item into view
  useEffect(() => {
    if (!menuRef.current || selectedIndex < 0) return;
    const selected = menuRef.current.querySelector("[data-selected]");
    selected?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  if (!state || filtered.length === 0) return null;

  // Clamp position to viewport
  let x = state.x;
  let y = state.y;
  if (x + 240 > window.innerWidth) x = window.innerWidth - 250;
  if (y + 300 > window.innerHeight) y = Math.max(4, window.innerHeight - 300);

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-50 min-w-[220px] max-h-[300px] overflow-y-auto glass-dropdown rounded-xl p-1"
      style={{ left: x, top: y }}
    >
      {filtered.map((cmd, i) => {
        const Icon = cmd.icon;
        const isSelected = i === selectedIndex;
        return (
          <button
            key={cmd.label}
            type="button"
            data-selected={isSelected ? "" : undefined}
            onClick={() => executeCommand(i)}
            onMouseEnter={() => setSelectedIndex(i)}
            className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-left text-[13px] transition-colors ${
              isSelected ? "bg-surface-raised text-primary" : "text-secondary hover:bg-surface-low"
            }`}
          >
            <span className="w-7 h-7 rounded-md flex items-center justify-center bg-brand/10 text-brand shrink-0">
              <Icon className="w-4 h-4" />
            </span>
            <span>{cmd.label}</span>
          </button>
        );
      })}
    </div>,
    document.body,
  );
}
