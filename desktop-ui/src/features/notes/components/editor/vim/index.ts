// index.ts — tiptap Extension wrapping @replit/codemirror-vim engine
//
// Architecture:
//   vim-engine.js → initVim(ProseMirrorAdapter) → Vim API
//   This file creates the tiptap Extension + ProseMirror plugins.

import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Editor } from "@tiptap/react";
import { Extension } from "@tiptap/react";
import { ProseMirrorAdapter } from "./ProseMirrorAdapter";
import type { VimModeChangeEvent } from "./types";

// @ts-expect-error — vim-engine.js is untyped
import { initVim } from "./vim-engine";

// ── Public types ────────────────────────────────────────────────────────

export type VimMode = "normal" | "insert" | "visual" | "visual-line" | "replace";

export interface VimModeOptions {
  onStateChange?: (state: { mode: VimMode }) => void;
  onOpenCommandLine?: (prefix: string) => void;
  enabled?: () => boolean;
}

// ── Plugin keys ─────────────────────────────────────────────────────────

const vimPluginKey = new PluginKey("vim");
const vimCursorKey = new PluginKey("vimCursor");
const vimSearchKey = new PluginKey("vimSearch");

// ── Vim API singleton ───────────────────────────────────────────────────

// biome-ignore lint/suspicious/noExplicitAny: vim.js is untyped
let VimApi: any = null;

function ensureVim() {
  if (!VimApi) {
    VimApi = initVim(ProseMirrorAdapter);
  }
  return VimApi;
}

// ── VimPlugin instance interface ────────────────────────────────────────

export interface VimPluginInstance extends Plugin {
  getAdapter: () => ProseMirrorAdapter | null;
  setSearchPattern: (pattern: string, direction: "forward" | "backward") => void;
  executeCommand: (cmd: string) => void;
}

export const VIM_SAVE_EVENT = "vim:save";

export function getVimPlugin(editor: Editor): VimPluginInstance | null {
  const plugin = vimPluginKey.get(editor.state);
  return plugin ? (plugin as unknown as VimPluginInstance) : null;
}

// ── Key handling ────────────────────────────────────────────────────────

// Special keys that vim.js expects wrapped in angle brackets: <Esc>, <CR>, <BS>, etc.
const SPECIAL_KEYS: Record<string, string> = {
  Escape: "Esc",
  Enter: "CR",
  Backspace: "BS",
  Delete: "Del",
  Tab: "Tab",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Ins",
  " ": "Space",
};

function vimKeyFromEvent(e: KeyboardEvent): string | null {
  // Skip pure modifier keys
  if (e.key === "Control" || e.key === "Alt" || e.key === "Shift" || e.key === "Meta") return null;

  const special = SPECIAL_KEYS[e.key];
  const key = special ?? e.key;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("C");
  if (e.altKey) parts.push("A");
  if (e.shiftKey && key.length > 1) parts.push("S");

  // Wrap in angle brackets if there are modifiers OR this is a special key
  if (parts.length > 0 || special) {
    return `<${[...parts, key].join("-")}>`;
  }
  return key;
}

// ── Create plugins ──────────────────────────────────────────────────────

function createVimPlugins(opts: Required<VimModeOptions>) {
  let adapter: ProseMirrorAdapter | null = null;
  let currentMode: VimMode = "normal";
  let searchOverlayQuery: RegExp | null = null;

  function mapVimMode(event: VimModeChangeEvent): VimMode {
    const { mode, subMode } = event;
    if (mode === "visual") {
      if (subMode === "linewise") return "visual-line";
      return "visual";
    }
    if (mode === "replace") return "replace";
    if (mode === "insert") return "insert";
    return "normal";
  }

  // ── Main vim plugin ──────────────────────────────────────────────

  const mainPlugin = new Plugin({
    key: vimPluginKey,

    view(editorView: EditorView) {
      const vim = ensureVim();
      adapter = new ProseMirrorAdapter(editorView);

      // Listen for vim events
      adapter.on("vim-mode-change", (event: VimModeChangeEvent) => {
        currentMode = mapVimMode(event);
        opts.onStateChange({ mode: currentMode });
      });

      adapter.on("vim-command-done", () => {
        // Trigger decoration update
        editorView.dispatch(editorView.state.tr);
      });

      adapter.on("dialog", (data: { template: unknown; callback: Function; options?: unknown }) => {
        // Determine prefix from template
        let prefix = ":";
        if (data.template instanceof HTMLElement) {
          const text = data.template.textContent || "";
          if (text.includes("/")) prefix = "/";
          else if (text.includes("?")) prefix = "?";
        }
        opts.onOpenCommandLine(prefix);
      });

      adapter.on("save", () => {
        document.dispatchEvent(new CustomEvent(VIM_SAVE_EVENT));
      });

      adapter.on("searchOverlayChange", (overlay: { query: RegExp } | null) => {
        searchOverlayQuery = overlay?.query ?? null;
        // Force decoration recalculation
        editorView.dispatch(editorView.state.tr);
      });

      // Enter vim mode
      vim.enterVimMode(adapter);

      // Register leader-key actions for editor features (\a, \f, \t, \i)
      const editorActions = [
        { key: "\\\\a", action: "annotate" },
        { key: "\\\\f", action: "flashcard" },
        { key: "\\\\t", action: "translate" },
        { key: "\\\\i", action: "ask-ai" },
      ];
      for (const { key, action } of editorActions) {
        vim.defineAction(`editor-${action}`, () => {
          window.dispatchEvent(new CustomEvent("editor-action", { detail: { action } }));
        });
        vim.mapCommand(key, "action", `editor-${action}`, {}, { context: "normal" });
        vim.mapCommand(key, "action", `editor-${action}`, {}, { context: "visual" });
      }

      return {
        update() {
          if (adapter) {
            adapter.view = editorView;
            adapter.onSelectionChange();
          }
        },
        destroy() {
          if (adapter) {
            vim.leaveVimMode(adapter);
            adapter = null;
          }
        },
      };
    },

    props: {
      handleKeyDown(_view: EditorView, event: KeyboardEvent): boolean {
        if (!opts.enabled() || !adapter) return false;

        const vim = ensureVim();
        const vimState = adapter.state.vim as
          | { insertMode?: boolean; visualMode?: boolean }
          | undefined;

        // In insert mode, only handle Escape
        if (vimState?.insertMode && event.key !== "Escape") {
          return false;
        }

        // Let Meta and Alt combos pass through (Cmd+C, ⌥A, ⌥F, etc.)
        if (event.metaKey || event.altKey) return false;

        const key = vimKeyFromEvent(event);
        if (!key) return false;

        try {
          const handled = vim.handleKey(adapter, key, "user");
          if (handled) {
            event.preventDefault();
            event.stopPropagation();
          }
          return handled;
        } catch (e) {
          console.warn("[vim] Key handling error:", e);
          return false;
        }
      },

      handleKeyPress(_view: EditorView, event: KeyboardEvent): boolean {
        if (!opts.enabled() || !adapter) return false;

        const vimState = adapter.state.vim as { insertMode?: boolean } | undefined;
        if (vimState?.insertMode) return false;

        // In normal/visual mode, consume keypress to prevent character insertion
        event.preventDefault();
        return true;
      },
    },
  }) as VimPluginInstance;

  // Attach public API
  mainPlugin.getAdapter = () => adapter;

  mainPlugin.setSearchPattern = (pattern: string, _direction: "forward" | "backward") => {
    if (!adapter) return;
    try {
      const query = new RegExp(pattern, "gi");
      adapter.addOverlay({ query });
    } catch (e) {
      console.warn("[vim] Invalid search pattern:", e);
    }
  };

  mainPlugin.executeCommand = (cmd: string) => {
    if (!adapter) return;
    if (cmd === "w" || cmd === "write") {
      document.dispatchEvent(new CustomEvent(VIM_SAVE_EVENT));
    } else {
      const vim = ensureVim();
      try {
        vim.handleEx?.(adapter, cmd);
      } catch (e) {
        console.warn("[vim] Ex command error:", e);
      }
    }
  };

  // ── Cursor decoration plugin ─────────────────────────────────────

  const cursorPlugin = new Plugin({
    key: vimCursorKey,
    props: {
      decorations(state) {
        if (!opts.enabled() || currentMode === "insert") {
          return DecorationSet.empty;
        }

        const pos = state.selection.head;
        const $pos = state.doc.resolve(pos);
        const end = Math.min(pos + 1, $pos.end());

        if (pos >= end) {
          // End-of-line: widget decoration
          return DecorationSet.create(state.doc, [
            Decoration.widget(pos, () => {
              const span = document.createElement("span");
              span.className = "vim-block-cursor vim-block-cursor--eol";
              span.textContent = "\u00A0";
              return span;
            }),
          ]);
        }

        return DecorationSet.create(state.doc, [
          Decoration.inline(pos, end, { class: "vim-block-cursor" }),
        ]);
      },
    },
  });

  // ── Search highlight plugin ──────────────────────────────────────

  const searchPlugin = new Plugin({
    key: vimSearchKey,
    props: {
      decorations(state) {
        if (!searchOverlayQuery) return DecorationSet.empty;

        try {
          const text = state.doc.textContent;
          const re = new RegExp(searchOverlayQuery.source, "gi");
          const decorations: Decoration[] = [];

          let m = re.exec(text);
          while (m !== null) {
            const from = findPMPosFromTextOffset(state.doc, m.index);
            const to = findPMPosFromTextOffset(state.doc, m.index + m[0].length);
            if (from !== null && to !== null && from < to) {
              decorations.push(
                Decoration.inline(from, to, {
                  class: "vim-search-match",
                }),
              );
            }
            if (m.index === re.lastIndex) re.lastIndex++;
            m = re.exec(text);
          }

          return DecorationSet.create(state.doc, decorations);
        } catch {
          return DecorationSet.empty;
        }
      },
    },
  });

  return [mainPlugin, cursorPlugin, searchPlugin];
}

/** Map a flat textContent offset to a ProseMirror document position */
function findPMPosFromTextOffset(
  doc: import("@tiptap/pm/model").Node,
  offset: number,
): number | null {
  let textIdx = 0;
  let result: number | null = null;

  doc.descendants((node, pos) => {
    if (result !== null) return false;
    if (node.isText && node.text) {
      if (textIdx + node.text.length > offset) {
        result = pos + (offset - textIdx);
        return false;
      }
      textIdx += node.text.length;
    }
    return true;
  });

  if (result === null && offset >= textIdx) {
    result = doc.content.size;
  }

  return result;
}

// ── tiptap Extension ────────────────────────────────────────────────────

export const VimModeExtension = Extension.create<VimModeOptions>({
  name: "vimMode",

  addOptions() {
    return {
      onStateChange: undefined,
      onOpenCommandLine: undefined,
      enabled: undefined,
    };
  },

  addProseMirrorPlugins() {
    const enabled = this.options.enabled ?? (() => false);
    return createVimPlugins({
      onStateChange: this.options.onStateChange ?? (() => {}),
      onOpenCommandLine: this.options.onOpenCommandLine ?? (() => {}),
      enabled,
    });
  },
});

export default VimModeExtension;
