import { useEffect } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import { executeItem } from "./useExecuteItem";

interface KeyboardNavOptions {
  onEnterChat: (query: string) => void;
  onExpandToMain: () => void;
  onHide: () => void;
}

export function useKeyboardNavigation({ onEnterChat, onExpandToMain, onHide }: KeyboardNavOptions) {
  const mode = useLauncherStore((s) => s.mode);

  useEffect(() => {
    if (mode === "chat") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      const store = useLauncherStore.getState();

      // When action menu is open, let the ActionMenu component handle keys
      if (store.actionMenuOpen) return;

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault();
          if (e.altKey && mode === "search") {
            // Alt+Down: navigate query history forward
            store.navigateHistory("down");
          } else {
            store.moveSelection(1);
          }
          break;
        }

        case "ArrowUp": {
          e.preventDefault();
          if (e.altKey && mode === "search") {
            // Alt+Up: navigate query history backward
            store.navigateHistory("up");
          } else {
            store.moveSelection(-1);
          }
          break;
        }

        case "Enter": {
          e.preventDefault();
          const item = store.results[store.selectedIndex];
          if (!item) {
            if (store.query.trim()) {
              onEnterChat(store.query);
            }
            return;
          }

          if (item.arguments && item.arguments.length > 0) {
            store.setArgModeItem(item);
            return;
          }

          executeItem(item, { onEnterChat, onExpandToMain, onHide });
          break;
        }

        case "Escape": {
          e.preventDefault();
          if (store.argModeItem) {
            store.setArgModeItem(null);
          } else if (store.mode === "detail") {
            store.setDetailItem(null);
            store.setMode("search");
          } else if (store.query) {
            store.setQuery("");
          } else {
            onHide();
          }
          break;
        }

        case "Tab": {
          e.preventDefault();
          const item = store.results[store.selectedIndex];
          if (item && store.mode === "search") {
            store.setDetailItem(item);
            store.setMode("detail");
          } else if (store.mode === "detail") {
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
            if (store.mode === "search" || store.mode === "detail") {
              const item = store.results[store.selectedIndex];
              if (item) {
                store.setActionMenuOpen(true);
              }
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
  }, [mode, onEnterChat, onExpandToMain, onHide]);
}
