import { ipc } from "@shared/hooks/useIpc";
import { useEffect } from "react";
import { useLauncherStore } from "../stores/launcherStore";

interface KeyboardNavOptions {
  onEnterChat: (query: string) => void;
  onExpandToMain: () => void;
  onHide: () => void;
}

export function useKeyboardNavigation({ onEnterChat, onExpandToMain, onHide }: KeyboardNavOptions) {
  const mode = useLauncherStore((s) => s.mode);

  useEffect(() => {
    if (mode === "chat") return;

    const handleKeyDown = async (e: KeyboardEvent) => {
      const store = useLauncherStore.getState();

      switch (e.key) {
        case "ArrowDown":
        case "ArrowUp": {
          e.preventDefault();
          store.moveSelection(e.key === "ArrowDown" ? 1 : -1);
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

          store.pushHistory(store.query);

          switch (item.kind.type) {
            case "aiChat":
              onEnterChat(item.kind.query);
              break;
            case "application":
              try {
                await ipc("launcher_open_app", { path: item.kind.path });
                await ipc("launcher_execute", {
                  itemId: item.id,
                  kind: "app",
                });
                onHide();
              } catch (err) {
                console.error("Failed to open app:", err);
              }
              break;
            case "systemCommand":
              try {
                await ipc("launcher_system_command", {
                  action: item.kind.action,
                });
                await ipc("launcher_execute", {
                  itemId: item.id,
                  kind: "command",
                });
                onHide();
              } catch (err) {
                console.error("Failed to execute system command:", err);
              }
              break;
            case "script":
              try {
                await ipc("launcher_run_script", { path: item.kind.path });
                await ipc("launcher_execute", {
                  itemId: item.id,
                  kind: "script",
                });
                onHide();
              } catch (err) {
                console.error("Failed to run script:", err);
              }
              break;
            case "clipboardEntry":
              try {
                await ipc("launcher_clipboard_paste", {
                  id: item.kind.entryId,
                });
                await ipc("launcher_execute", {
                  itemId: item.id,
                  kind: "clipboard",
                });
                onHide();
              } catch (err) {
                console.error("Failed to paste clipboard:", err);
              }
              break;
            case "calculator":
              try {
                await navigator.clipboard.writeText(String(item.kind.result));
                onHide();
              } catch (err) {
                console.error("Failed to copy:", err);
              }
              break;
            default:
              // task, note, calendar — navigate to them
              await ipc("launcher_execute", {
                itemId: item.id,
                kind: item.kind.type,
              });
              onExpandToMain();
              break;
          }
          break;
        }

        case "Escape": {
          e.preventDefault();
          if (store.query) {
            store.setQuery("");
          } else {
            onHide();
          }
          break;
        }

        case "Tab": {
          // Reserved for future detail panel
          e.preventDefault();
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
            onExpandToMain();
          }
          break;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [mode, onEnterChat, onExpandToMain, onHide]);
}
