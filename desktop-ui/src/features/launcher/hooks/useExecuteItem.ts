import { ipc } from "@shared/hooks/useIpc";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

interface ExecuteItemOptions {
  onEnterChat: (query: string) => void;
  onExpandToMain: () => void;
  onHide: () => void;
}

export function executeItem(item: LauncherItem, options: ExecuteItemOptions) {
  const { onEnterChat, onExpandToMain, onHide } = options;

  useLauncherStore.getState().pushHistory(useLauncherStore.getState().query);

  switch (item.kind.type) {
    case "aiChat":
      onEnterChat(item.kind.query);
      break;
    case "application":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: "app" }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to open app:", err));
      break;
    case "systemCommand":
      ipc("launcher_system_command", { action: item.kind.action })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: "command" }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to execute system command:", err));
      break;
    case "script":
      ipc("launcher_run_script", { path: item.kind.path })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: "script" }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to run script:", err));
      break;
    case "clipboardEntry":
      ipc("launcher_clipboard_paste", { id: item.kind.entryId })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: "clipboard" }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to paste clipboard:", err));
      break;
    case "calculator":
      navigator.clipboard
        .writeText(String(item.kind.result))
        .then(() => onHide())
        .catch((err) => console.error("Failed to copy:", err));
      break;
    case "file":
    case "contentMatch":
    case "gitRepo":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: item.kind.type }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to open:", err));
      break;
    case "bookmark":
    case "browserHistory":
    case "urlNavigation":
      ipc("launcher_open_app", { path: item.kind.url })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: item.kind.type }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to open URL:", err));
      break;
    case "systemPref":
      ipc("launcher_open_app", { path: `x-apple.systempreferences:${item.kind.paneId}` })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: "pref" }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to open pref:", err));
      break;
    case "runningApp":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() => ipc("launcher_execute", { itemId: item.id, kind: "running_app" }))
        .then(() => onHide())
        .catch((err) => console.error("Failed to focus app:", err));
      break;
    case "sshHost":
      {
        const sshCmd = item.kind.user
          ? `ssh://${item.kind.user}@${item.kind.host}`
          : `ssh://${item.kind.host}`;
        ipc("launcher_open_app", { path: sshCmd })
          .then(() => ipc("launcher_execute", { itemId: item.id, kind: "ssh" }))
          .then(() => onHide())
          .catch((err) => console.error("Failed to open SSH:", err));
      }
      break;
    case "contact":
      {
        const uri = item.kind.email
          ? `mailto:${item.kind.email}`
          : item.kind.phone
            ? `tel:${item.kind.phone}`
            : null;
        if (uri) {
          ipc("launcher_open_app", { path: uri })
            .then(() => ipc("launcher_execute", { itemId: item.id, kind: "contact" }))
            .then(() => onHide())
            .catch((err) => console.error("Failed to open contact:", err));
        }
      }
      break;
    case "brewPackage":
      navigator.clipboard
        .writeText(item.kind.name)
        .then(() => onHide())
        .catch((err) => console.error("Failed to copy package name:", err));
      break;
    default:
      // task, note, calendar — navigate to them in main window
      ipc("launcher_execute", { itemId: item.id, kind: item.kind.type })
        .then(() => onExpandToMain())
        .catch((err) => console.error("Failed to execute:", err));
      break;
  }
}
