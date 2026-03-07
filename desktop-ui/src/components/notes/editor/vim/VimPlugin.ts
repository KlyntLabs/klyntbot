import { Plugin, PluginKey, TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import {
  cmdAppend,
  cmdAppendLineEnd,
  cmdChangeLine,
  cmdDeleteChar,
  cmdDeleteCharBefore,
  cmdDeleteLine,
  cmdInsert,
  cmdInsertLineStart,
  cmdJoinLines,
  cmdOpenAbove,
  cmdOpenBelow,
  cmdPaste,
  cmdPasteBefore,
  cmdRedo,
  cmdReplaceChar,
  cmdToggleCaseChar,
  cmdUndo,
  cmdYankLine,
} from "./commands";
import { MOTIONS, motionFindChar } from "./motions";
import {
  operatorChange,
  operatorDelete,
  operatorIndent,
  operatorLowercase,
  operatorUppercase,
  operatorYank,
} from "./operators";
import { buildFlatTextMap, clampPos, cursorPos, lineEnd, lineStart } from "./positions";
import { findNextMatch } from "./search";
import { findTextObject } from "./textObjects";
import type { Operator, SearchDirection, VimState } from "./VimState";
import {
  accumulateCount,
  createVimState,
  effectiveCount,
  enterNormalMode,
  enterVisualLineMode,
  enterVisualMode,
  resetOperatorState,
} from "./VimState";

export interface VimPluginOptions {
  onStateChange: (state: VimState) => void;
  onOpenCommandLine: (prefix: string) => void;
  getEnabled: () => boolean;
}

/** Extended Plugin with typed public API for vim state access. */
export interface VimPluginInstance extends Plugin {
  getVimState: () => VimState;
  setSearchPattern: (pattern: string, direction: SearchDirection) => void;
  executeCommand: (cmd: string) => void;
}

export const vimPluginKey = new PluginKey("vim");
export const VIM_SAVE_EVENT = "vim:save";

export function createVimPlugin(opts: VimPluginOptions): VimPluginInstance {
  let vim = createVimState();

  function updateState(newVim: VimState): void {
    vim = newVim;
    opts.onStateChange(vim);
  }

  function recordKey(key: string): void {
    if (vim.mode !== "insert") {
      vim.recording.push(key);
    }
  }

  /**
   * Apply a pending operator over a range.
   * Returns true if an operator was applied (and state was updated).
   */
  function applyOperator(view: EditorView, from: number, to: number, linewise: boolean): boolean {
    const op = vim.operator;
    if (!op) return false;

    const orderedFrom = Math.min(from, to);
    const orderedTo = Math.max(from, to);

    switch (op) {
      case "d":
        updateState(operatorDelete(view, orderedFrom, orderedTo, vim, linewise));
        return true;
      case "c":
        updateState(operatorChange(view, orderedFrom, orderedTo, vim));
        return true;
      case "y":
        updateState(operatorYank(view, orderedFrom, orderedTo, vim));
        return true;
      case ">":
        updateState(operatorIndent(view, orderedFrom, orderedTo, vim, false));
        return true;
      case "<":
        updateState(operatorIndent(view, orderedFrom, orderedTo, vim, true));
        return true;
      case "gu":
        operatorLowercase(view, orderedFrom, orderedTo);
        updateState(enterNormalMode(vim));
        return true;
      case "gU":
        operatorUppercase(view, orderedFrom, orderedTo);
        updateState(enterNormalMode(vim));
        return true;
    }
    return false;
  }

  /**
   * Move cursor and handle visual selection extension or operator application.
   */
  function handleMotion(view: EditorView, targetPos: number, linewise: boolean): void {
    const head = cursorPos(view.state);

    if (vim.operator) {
      // Apply operator over the motion range
      applyOperator(view, head, targetPos, linewise);
      return;
    }

    if (vim.mode === "visual" || vim.mode === "visual-line") {
      // Extend visual selection
      const anchor = vim.visualAnchor ?? head;
      let selFrom: number;
      let selTo: number;

      if (vim.mode === "visual-line") {
        const anchorLineStart = lineStart(view.state, anchor);
        const anchorLineEnd = lineEnd(view.state, anchor);
        const targetLineStart = lineStart(view.state, targetPos);
        const targetLineEnd = lineEnd(view.state, targetPos);
        selFrom = Math.min(anchorLineStart, targetLineStart);
        selTo = Math.max(anchorLineEnd, targetLineEnd);
      } else {
        selFrom = Math.min(anchor, targetPos);
        selTo = Math.max(anchor, targetPos);
        // In visual mode, selection is inclusive of the cursor character
        // but must not extend past the current textblock boundary
        if (selTo < view.state.doc.content.size) {
          const $to = view.state.doc.resolve(selTo);
          const blockEnd = $to.end($to.depth);
          selTo = Math.min(selTo + 1, blockEnd);
        }
      }

      const tr = view.state.tr
        .setSelection(TextSelection.create(view.state.doc, selFrom, selTo))
        .scrollIntoView();
      view.dispatch(tr);
      updateState({ ...resetOperatorState(vim), visualAnchor: anchor });
      return;
    }

    // Normal mode: just move cursor
    const clamped = clampPos(view.state, targetPos);
    const tr = view.state.tr
      .setSelection(TextSelection.create(view.state.doc, clamped))
      .scrollIntoView();
    view.dispatch(tr);
    updateState(resetOperatorState(vim));
  }

  function handleKeyDown(view: EditorView, event: KeyboardEvent): boolean {
    // When vim mode is disabled, pass all keys through to normal editing
    if (!opts.getEnabled()) return false;

    const key = event.key;

    // -----------------------------------------------------------------------
    // INSERT MODE
    // -----------------------------------------------------------------------
    if (vim.mode === "insert") {
      if (key === "Escape") {
        // Return to normal mode, move cursor back 1 if possible
        const head = cursorPos(view.state);
        const start = lineStart(view.state, head);
        if (head > start) {
          const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, head - 1));
          view.dispatch(tr);
        }
        updateState(enterNormalMode(vim));
        return true;
      }
      // Let all other keys pass through to default editing
      return false;
    }

    // -----------------------------------------------------------------------
    // NORMAL / VISUAL MODES
    // -----------------------------------------------------------------------

    // 1. Ignore modifier-only keys
    if (key === "Control" || key === "Meta" || key === "Alt" || key === "Shift") {
      return false;
    }

    // 2. Handle Ctrl+R (redo)
    if (event.ctrlKey && key === "r") {
      recordKey("C-r");
      updateState(cmdRedo(view, vim));
      return true;
    }

    // 3. Let Meta/Ctrl combos pass through (browser shortcuts like Cmd+C, Cmd+V)
    if (event.metaKey || event.ctrlKey) {
      return false;
    }

    // 4. If awaitingChar is set: handle f/F/t/T char or r char
    if (vim.awaitingChar) {
      recordKey(key);
      const awaiting = vim.awaitingChar;

      if (awaiting === "r") {
        // Replace character
        updateState(cmdReplaceChar(view, vim, key));
        return true;
      }

      // f/F/t/T — find char motion
      const forward = awaiting === "f" || awaiting === "t";
      const till = awaiting === "t" || awaiting === "T";
      const count = effectiveCount(vim);
      const result = motionFindChar(view.state, key, forward, till, count);

      vim = { ...vim, awaitingChar: null };
      handleMotion(view, result.pos, false);
      return true;
    }

    // 5. If gPending: handle gg, gu, gU
    if (vim.gPending) {
      recordKey(key);

      if (key === "g") {
        // gg — go to start of document (or line N with count)
        const motionGG = MOTIONS.gg;
        if (motionGG) {
          vim = { ...vim, gPending: false };
          const result = motionGG(view, view.state, vim);
          handleMotion(view, result.pos, result.linewise ?? false);
        } else {
          updateState({ ...resetOperatorState(vim), gPending: false });
        }
        return true;
      }

      if (key === "u") {
        // gu — lowercase operator
        if (vim.operator) {
          // gu applied with pending operator — not standard, reset
          updateState({ ...resetOperatorState(vim), gPending: false });
        } else {
          updateState({ ...vim, gPending: false, operator: "gu" as Operator });
        }
        return true;
      }

      if (key === "U") {
        // gU — uppercase operator
        if (vim.operator) {
          updateState({ ...resetOperatorState(vim), gPending: false });
        } else {
          updateState({ ...vim, gPending: false, operator: "gU" as Operator });
        }
        return true;
      }

      // Unknown g-command, cancel
      updateState({ ...resetOperatorState(vim), gPending: false });
      return true;
    }

    // 6. If operator is set and key is 'i' or 'a': text object scope
    if (vim.operator && (key === "i" || key === "a")) {
      recordKey(key);
      const scope = key as "i" | "a";

      // Use a one-shot DOM listener to capture the text object target key
      const onceHandler = (e: KeyboardEvent) => {
        e.preventDefault();
        e.stopPropagation();
        document.removeEventListener("keydown", onceHandler, true);

        recordKey(e.key);

        const map = buildFlatTextMap(view.state);
        const head = cursorPos(view.state);
        const flatIdx = map.toFlat.get(head) ?? 0;
        const range = findTextObject(map.text, flatIdx, scope, e.key);

        if (range) {
          const pmFrom =
            range.from < map.toPM.length ? map.toPM[range.from] : view.state.doc.content.size;
          const pmTo =
            range.to < map.toPM.length ? map.toPM[range.to] : view.state.doc.content.size;
          applyOperator(view, pmFrom, pmTo, false);
        } else {
          updateState(resetOperatorState(vim));
        }
      };
      document.addEventListener("keydown", onceHandler, true);
      return true;
    }

    // 7. Count digits (1-9, or 0 if count already started)
    if (/^[1-9]$/.test(key) || (key === "0" && vim.count !== null)) {
      recordKey(key);
      updateState(accumulateCount(vim, Number.parseInt(key, 10)));
      return true;
    }

    // 8. Operators: d, c, y — if doubled, operate on line; otherwise set pending
    if (key === "d" || key === "c" || key === "y") {
      recordKey(key);

      if (vim.operator === key) {
        // Doubled: dd, cc, yy — operate on current line(s)
        switch (key) {
          case "d":
            updateState(cmdDeleteLine(view, vim));
            break;
          case "c":
            updateState(cmdChangeLine(view, vim));
            break;
          case "y":
            updateState(cmdYankLine(view, vim));
            break;
        }
        return true;
      }

      // Set as pending operator
      updateState({ ...vim, operator: key as Operator });
      return true;
    }

    // 9. Indent operators: >, <
    if (key === ">" || key === "<") {
      recordKey(key);

      if (vim.operator === key) {
        // Doubled: >> or << — indent/outdent current line
        const head = cursorPos(view.state);
        const from = lineStart(view.state, head);
        const to = lineEnd(view.state, head);
        updateState(operatorIndent(view, from, to, vim, key === "<"));
        return true;
      }

      updateState({ ...vim, operator: key as Operator });
      return true;
    }

    // 10. g prefix
    if (key === "g") {
      recordKey(key);
      updateState({ ...vim, gPending: true });
      return true;
    }

    // 11. Awaiting char triggers: f, F, t, T, r
    if (key === "f" || key === "F" || key === "t" || key === "T" || key === "r") {
      recordKey(key);
      updateState({ ...vim, awaitingChar: key as "f" | "F" | "t" | "T" | "r" });
      return true;
    }

    // 12. Motion keys: look up in MOTIONS registry
    if (key in MOTIONS) {
      recordKey(key);
      const motionFn = MOTIONS[key];
      const result = motionFn(view, view.state, vim);
      handleMotion(view, result.pos, result.linewise ?? false);
      return true;
    }

    // 13. Direct commands
    recordKey(key);

    switch (key) {
      // Insert mode entries
      case "i":
        updateState(cmdInsert(view, vim));
        return true;
      case "a":
        updateState(cmdAppend(view, vim));
        return true;
      case "I":
        updateState(cmdInsertLineStart(view, vim));
        return true;
      case "A":
        updateState(cmdAppendLineEnd(view, vim));
        return true;
      case "o":
        updateState(cmdOpenBelow(view, vim));
        return true;
      case "O":
        updateState(cmdOpenAbove(view, vim));
        return true;

      // Delete/change chars
      case "x":
        updateState(cmdDeleteChar(view, vim));
        return true;
      case "X":
        updateState(cmdDeleteCharBefore(view, vim));
        return true;

      // Paste
      case "p":
        updateState(cmdPaste(view, vim));
        return true;
      case "P":
        updateState(cmdPasteBefore(view, vim));
        return true;

      // Join lines
      case "J":
        updateState(cmdJoinLines(view, vim));
        return true;

      // Toggle case
      case "~":
        updateState(cmdToggleCaseChar(view, vim));
        return true;

      // Undo
      case "u":
        updateState(cmdUndo(view, vim));
        return true;

      // Visual mode
      case "v": {
        if (vim.mode === "visual") {
          // Toggle off — collapse selection
          const head = cursorPos(view.state);
          const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, head));
          view.dispatch(tr);
          updateState(enterNormalMode(vim));
        } else {
          const head = cursorPos(view.state);
          updateState(enterVisualMode(vim, head));
        }
        return true;
      }
      case "V": {
        if (vim.mode === "visual-line") {
          const head = cursorPos(view.state);
          const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, head));
          view.dispatch(tr);
          updateState(enterNormalMode(vim));
        } else {
          const head = cursorPos(view.state);
          updateState(enterVisualLineMode(vim, head));
        }
        return true;
      }

      // Dot repeat
      case ".": {
        if (vim.lastAction && vim.lastAction.keys.length > 0) {
          const keysToReplay = [...vim.lastAction.keys];
          // Temporarily clear lastAction to avoid infinite recursion
          const savedAction = vim.lastAction;
          for (const k of keysToReplay) {
            const fakeEvent = new KeyboardEvent("keydown", {
              key: k,
              bubbles: true,
            });
            handleKeyDown(view, fakeEvent);
          }
          // Restore the lastAction so subsequent dots replay the same thing
          vim = { ...vim, lastAction: savedAction };
          updateState(vim);
        }
        return true;
      }

      // Search
      case "/": {
        opts.onOpenCommandLine("/");
        updateState(resetOperatorState(vim));
        return true;
      }

      // Command line
      case ":": {
        opts.onOpenCommandLine(":");
        updateState(resetOperatorState(vim));
        return true;
      }

      // Search next/previous
      case "n": {
        if (vim.searchPattern) {
          const map = buildFlatTextMap(view.state);
          const head = cursorPos(view.state);
          const flatIdx = map.toFlat.get(head) ?? 0;
          const matchIdx = findNextMatch(map.text, vim.searchPattern, flatIdx, vim.searchDirection);
          if (matchIdx !== null && matchIdx < map.toPM.length) {
            const pmPos = map.toPM[matchIdx];
            const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, pmPos));
            view.dispatch(tr.scrollIntoView());
          }
        }
        updateState(resetOperatorState(vim));
        return true;
      }
      case "N": {
        if (vim.searchPattern) {
          const map = buildFlatTextMap(view.state);
          const head = cursorPos(view.state);
          const flatIdx = map.toFlat.get(head) ?? 0;
          const reverseDir: "forward" | "backward" =
            vim.searchDirection === "forward" ? "backward" : "forward";
          const matchIdx = findNextMatch(map.text, vim.searchPattern, flatIdx, reverseDir);
          if (matchIdx !== null && matchIdx < map.toPM.length) {
            const pmPos = map.toPM[matchIdx];
            const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, pmPos));
            view.dispatch(tr.scrollIntoView());
          }
        }
        updateState(resetOperatorState(vim));
        return true;
      }

      // Marks: m{char}
      case "m": {
        const onceHandler = (e: KeyboardEvent) => {
          e.preventDefault();
          e.stopPropagation();
          document.removeEventListener("keydown", onceHandler, true);
          const markName = e.key;
          const head = cursorPos(view.state);
          updateState({
            ...resetOperatorState(vim),
            marks: { ...vim.marks, [markName]: head },
          });
        };
        document.addEventListener("keydown", onceHandler, true);
        return true;
      }

      // Jump to mark: '{char}
      case "'": {
        const onceHandler = (e: KeyboardEvent) => {
          e.preventDefault();
          e.stopPropagation();
          document.removeEventListener("keydown", onceHandler, true);
          const markName = e.key;
          const markPos = vim.marks[markName];
          if (markPos !== undefined) {
            const clamped = clampPos(view.state, markPos);
            const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, clamped));
            view.dispatch(tr.scrollIntoView());
          }
          updateState(resetOperatorState(vim));
        };
        document.addEventListener("keydown", onceHandler, true);
        return true;
      }

      // Escape in normal/visual mode
      case "Escape": {
        if (vim.mode === "visual" || vim.mode === "visual-line") {
          // Collapse selection to cursor head
          const head = cursorPos(view.state);
          const tr = view.state.tr
            .setSelection(TextSelection.create(view.state.doc, head))
            .scrollIntoView();
          view.dispatch(tr);
        }
        updateState(enterNormalMode(vim));
        return true;
      }

      default:
        // Unknown key — consume it to prevent typing in normal mode
        return true;
    }
  }

  const plugin = new Plugin({
    key: vimPluginKey,
    props: {
      handleKeyDown(view, event) {
        return handleKeyDown(view, event);
      },
    },
  }) as VimPluginInstance;

  // Attach public API to the plugin instance
  plugin.getVimState = (): VimState => vim;

  plugin.setSearchPattern = (pattern: string, direction: SearchDirection): void => {
    vim = { ...vim, searchPattern: pattern, searchDirection: direction };
    opts.onStateChange(vim);
  };

  plugin.executeCommand = (cmd: string): void => {
    if (cmd === "w" || cmd === "write") {
      document.dispatchEvent(new CustomEvent(VIM_SAVE_EVENT));
    }
  };

  return plugin;
}
