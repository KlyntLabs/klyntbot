// `executeItem` invokes Tauri commands that are pure side effects (open
// app, paste clipboard, focus window). It deliberately does not go through
// useTauriMutation because there's no cache state to invalidate; promoting
// it would just add a layer for no benefit.

import { ipc } from "@/utils/tauri-bridge";
import { parseDurationToEndsAt } from "../lib/parseDuration";
import { showError } from "../lib/showError";
import type { LauncherStoreApi } from "../store";
import type { LauncherExecuteResult, LauncherItem } from "../types";

export interface ExecuteItemOptions {
  store: LauncherStoreApi;
  onEnterChat: (query: string) => void;
  onExpandToMain: () => void;
  onHide: () => void;
  onNeedsOnboarding?: (retryFn: () => void) => void;
}

async function hideAndBadge(
  item: LauncherItem,
  result: LauncherExecuteResult,
  onHide: () => void,
): Promise<void> {
  onHide();
  if (item.noView && result.message) {
    await ipc("show_status_badge", {
      text: result.message,
      kind: result.badge,
      durationMs: 2000,
    });
  }
}

export function executeItem(
  item: LauncherItem,
  options: ExecuteItemOptions,
  args: Record<string, string> = {},
) {
  const { store, onEnterChat, onExpandToMain, onHide } = options;
  store.pushHistory(store.getState().query);

  switch (item.kind.type) {
    case "aiChat":
      onEnterChat(item.kind.query);
      break;
    case "application":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "app",
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't open app:", err));
      break;
    case "systemCommand":
      if (item.kind.action === "toggleDoNotDisturb") {
        const endsAt = parseDurationToEndsAt(args.duration ?? "");
        if (endsAt !== null) {
          const activate = (resolvedEndsAt: string) => {
            ipc<boolean>("focus_shortcuts_installed")
              .then((installed) => {
                if (!installed) {
                  options.onNeedsOnboarding?.(() => activate(resolvedEndsAt));
                  return;
                }
                ipc("focus_activate", { mode: "dnd", endsAt: resolvedEndsAt })
                  .then(() =>
                    ipc<LauncherExecuteResult>("launcher_execute", {
                      itemId: item.id,
                      kind: "system",
                    }),
                  )
                  .then((result) => hideAndBadge(item, result, onHide))
                  .catch((err) => {
                    showError("Couldn't activate focus:", err);
                    options.onNeedsOnboarding?.(() => activate(resolvedEndsAt));
                  });
              })
              .catch((err) => showError("Couldn't check focus shortcuts:", err));
          };
          activate(endsAt);
        } else {
          ipc("launcher_system_command", { action: item.kind.action, args })
            .then(() =>
              ipc<LauncherExecuteResult>("launcher_execute", {
                itemId: item.id,
                kind: "system",
              }),
            )
            .then((result) => hideAndBadge(item, result, onHide))
            .catch((err) => showError("Couldn't run command:", err));
        }
      } else {
        ipc("launcher_system_command", { action: item.kind.action, args })
          .then(() =>
            ipc<LauncherExecuteResult>("launcher_execute", {
              itemId: item.id,
              kind: "system",
              args,
            }),
          )
          .then((result) => hideAndBadge(item, result, onHide))
          .catch((err) => showError("Couldn't run command:", err));
      }
      break;
    case "script":
      ipc("launcher_run_script", { path: item.kind.path, args })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "script",
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't run script:", err));
      break;
    case "clipboardEntry":
      ipc("launcher_clipboard_paste", { id: item.kind.entryId })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "clipboard",
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't paste:", err));
      break;
    case "calculator":
      navigator.clipboard
        .writeText(String(item.kind.result))
        .then(() => onHide())
        .catch((err) => showError("Couldn't copy:", err));
      break;
    case "file":
    case "contentMatch":
    case "gitRepo":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: item.kind.type,
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't open:", err));
      break;
    case "bookmark":
    case "browserHistory":
    case "urlNavigation":
      ipc("launcher_open_app", { path: item.kind.url })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: item.kind.type,
            args,
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't open URL:", err));
      break;
    case "systemPref":
      ipc("launcher_open_app", {
        path: `x-apple.systempreferences:${item.kind.paneId}`,
      })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "pref",
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't open setting:", err));
      break;
    case "runningApp":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "running_app",
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't focus app:", err));
      break;
    case "sshHost": {
      const sshCmd = item.kind.user
        ? `ssh://${item.kind.user}@${item.kind.host}`
        : `ssh://${item.kind.host}`;
      ipc("launcher_open_app", { path: sshCmd })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "ssh",
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't open SSH:", err));
      break;
    }
    case "contact": {
      const uri = item.kind.email
        ? `mailto:${item.kind.email}`
        : item.kind.phone
          ? `tel:${item.kind.phone}`
          : null;
      if (uri) {
        ipc("launcher_open_app", { path: uri })
          .then(() =>
            ipc<LauncherExecuteResult>("launcher_execute", {
              itemId: item.id,
              kind: "contact",
              args,
            }),
          )
          .then((result) => hideAndBadge(item, result, onHide))
          .catch((err) => showError("Couldn't open contact:", err));
      }
      break;
    }
    case "windowAction":
      ipc("launcher_window_action", { action: item.kind.action })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "window",
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => showError("Couldn't apply layout:", err));
      break;
    case "brewPackage":
      navigator.clipboard
        .writeText(item.kind.name)
        .then(() => onHide())
        .catch((err) => showError("Couldn't copy package name:", err));
      break;
    default:
      ipc("launcher_execute", { itemId: item.id, kind: item.kind.type })
        .then(() => onExpandToMain())
        .catch((err) => showError("Couldn't execute:", err));
      break;
  }
}
