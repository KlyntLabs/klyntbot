import { matchesShortcut } from "@utils/shortcuts";
import { useEffect } from "react";

const EDITABLE_SELECTOR =
  'input, textarea, select, [contenteditable=""], [contenteditable="true"], [role="textbox"]';

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.isContentEditable) {
    return true;
  }
  return Boolean(target.closest(EDITABLE_SELECTOR));
}

export interface GlobalShortcutConfig {
  /** Shortcut string parsed by matchesShortcut (e.g. "cmd+k"). Null disables. */
  shortcut: string | null;
  /** Called when the shortcut fires. Receives the KeyboardEvent if you need preventDefault. */
  handler: (event: KeyboardEvent) => void;
  /** If true, shortcut fires even when an input/textarea is focused. Default false. */
  allowInInput?: boolean;
}

export interface UseGlobalShortcutOptions {
  /** Global shortcut configs. */
  shortcuts: GlobalShortcutConfig[];
  /** If false, no listeners are registered. Default true. */
  enabled?: boolean;
}

/**
 * Subscribe to global keyboard shortcuts with consistent guards
 * (defaultPrevented, repeat, editable-target checks).
 *
 * Example:
 *   useGlobalShortcut({
 *     shortcuts: [
 *       { shortcut: "cmd+k", handler: (e) => { e.preventDefault(); openPalette(); } },
 *       { shortcut: "escape", handler: closeModal, allowInInput: true },
 *     ],
 *     enabled: isOpen,
 *   });
 */
export function useGlobalShortcut({ shortcuts, enabled = true }: UseGlobalShortcutOptions) {
  useEffect(() => {
    if (!enabled || shortcuts.length === 0) {
      return;
    }

    const activeShortcuts = shortcuts.filter(
      (s): s is GlobalShortcutConfig & { shortcut: string } => Boolean(s.shortcut),
    );
    if (activeShortcuts.length === 0) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.repeat) {
        return;
      }

      for (const config of activeShortcuts) {
        if (!config.allowInInput && isEditableTarget(event.target)) {
          continue;
        }
        if (matchesShortcut(event, config.shortcut)) {
          config.handler(event);
          return;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [enabled, shortcuts]);
}
