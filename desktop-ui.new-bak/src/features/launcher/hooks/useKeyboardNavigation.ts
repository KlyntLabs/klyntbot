import { useEffect } from "react";
import { useLauncherApi, useLauncherState } from "../store";
import { executeItem } from "./useExecuteItem";

interface KeyboardNavOptions {
  onEnterChat: (query: string) => void;
  onExpandToMain: () => void;
  onHide: () => void;
}

export function useKeyboardNavigation({ onEnterChat, onExpandToMain, onHide }: KeyboardNavOptions) {
  const mode = useLauncherState((s) => s.mode);
  const store = useLauncherApi();

  useEffect(() => {
    if (mode === "chat") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      const s = store.getState();
      if (s.actionMenuOpen) return;

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          if (e.altKey && mode === "search") store.navigateHistory("down");
          else store.moveSelection(1);
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          if (e.altKey && mode === "search") store.navigateHistory("up");
          else store.moveSelection(-1);
          break;
        }
        case "Enter": {
          e.preventDefault();
          const item = s.results[s.selectedIndex];
          if (!item) {
            if (s.query.trim()) onEnterChat(s.query);
            return;
          }
          if (item.arguments && item.arguments.length > 0) {
            store.setArgModeItem(item);
            return;
          }
          executeItem(item, { store, onEnterChat, onExpandToMain, onHide });
          break;
        }
        case "Escape": {
          e.preventDefault();
          if (s.argModeItem) store.setArgModeItem(null);
          else if (s.mode === "detail") {
            store.setDetailItem(null);
            store.setMode("search");
          } else if (s.query) {
            store.setQuery("");
          } else {
            onHide();
          }
          break;
        }
        case "Tab": {
          e.preventDefault();
          const item = s.results[s.selectedIndex];
          if (item && s.mode === "search") {
            store.setDetailItem(item);
            store.setMode("detail");
          } else if (s.mode === "detail") {
            store.setDetailItem(null);
            store.setMode("search");
          }
          break;
        }
        case "j": {
          if (e.ctrlKey) {
            e.preventDefault();
            store.moveSelection(1);
          }
          break;
        }
        case "k": {
          if (e.ctrlKey) {
            e.preventDefault();
            store.moveSelection(-1);
          } else if (e.metaKey) {
            e.preventDefault();
            if (s.mode === "search" || s.mode === "detail") {
              const item = s.results[s.selectedIndex];
              if (item) store.setActionMenuOpen(true);
            } else {
              onExpandToMain();
            }
          }
          break;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [mode, store, onEnterChat, onExpandToMain, onHide]);
}
