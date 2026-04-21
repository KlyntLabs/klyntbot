import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { parseDurationToEndsAt } from "../lib/parseDuration";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherExecuteResult, LauncherItem } from "../types";

interface ExecuteItemOptions {
  onEnterChat: (query: string) => void;
  onExpandToMain: () => void;
  onHide: () => void;
  /** Called when focus_activate fails because shortcuts aren't installed. */
  onNeedsOnboarding?: (retryFn: () => void) => void;
}

async function hideAndBadge(
  item: LauncherItem,
  result: LauncherExecuteResult,
  onHide: () => void,
): Promise<void> {
  onHide();
  if (item.noView && result.message) {
    await ipc("show_status_badge", { text: result.message, kind: result.badge, durationMs: 2000 });
  }
}

export function executeItem(
  item: LauncherItem,
  options: ExecuteItemOptions,
  args: Record<string, string> = {},
) {
  const { onEnterChat, onExpandToMain, onHide } = options;

  useLauncherStore.getState().pushHistory(useLauncherStore.getState().query);

  switch (item.kind.type) {
    case "aiChat":
      onEnterChat(item.kind.query);
      break;
    case "application":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", { itemId: item.id, kind: "app", args }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => console.error("Failed to open app:", err));
      break;
    case "systemCommand":
      if (item.kind.action === "toggleDoNotDisturb") {
        const endsAt = parseDurationToEndsAt(args.duration ?? "");
        if (endsAt !== null) {
          const activate = (resolvedEndsAt: string) => {
            ipc("focus_shortcuts_installed")
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
                      args,
                    }),
                  )
                  .then((result) => {
                    invalidateQueries("focus_active");
                    hideAndBadge(item, result, onHide);
                  })
                  .catch((err) => {
                    console.error("focus_activate failed:", err);
                    options.onNeedsOnboarding?.(() => activate(resolvedEndsAt));
                  });
              })
              .catch((err) => console.error("focus_shortcuts_installed failed:", err));
          };
          activate(endsAt);
        } else {
          // Unparseable duration — fall back to legacy system command path.
          ipc("launcher_system_command", { action: item.kind.action, args })
            .then(() =>
              ipc<LauncherExecuteResult>("launcher_execute", {
                itemId: item.id,
                kind: "system",
                args,
              }),
            )
            .then((result) => hideAndBadge(item, result, onHide))
            .catch((err) => console.error("Failed to execute system command:", err));
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
          .catch((err) => console.error("Failed to execute system command:", err));
      }
      break;
    case "script":
      ipc("launcher_run_script", { path: item.kind.path, args })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "script",
            args,
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => console.error("Failed to run script:", err));
      break;
    case "clipboardEntry":
      ipc("launcher_clipboard_paste", { id: item.kind.entryId })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "clipboard",
            args,
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
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
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: item.kind.type,
            args,
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => console.error("Failed to open:", err));
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
        .catch((err) => console.error("Failed to open URL:", err));
      break;
    case "systemPref":
      ipc("launcher_open_app", { path: `x-apple.systempreferences:${item.kind.paneId}` })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", { itemId: item.id, kind: "pref", args }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => console.error("Failed to open pref:", err));
      break;
    case "runningApp":
      ipc("launcher_open_app", { path: item.kind.path })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "running_app",
            args,
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => console.error("Failed to focus app:", err));
      break;
    case "sshHost":
      {
        const sshCmd = item.kind.user
          ? `ssh://${item.kind.user}@${item.kind.host}`
          : `ssh://${item.kind.host}`;
        ipc("launcher_open_app", { path: sshCmd })
          .then(() =>
            ipc<LauncherExecuteResult>("launcher_execute", { itemId: item.id, kind: "ssh", args }),
          )
          .then((result) => hideAndBadge(item, result, onHide))
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
            .then(() =>
              ipc<LauncherExecuteResult>("launcher_execute", {
                itemId: item.id,
                kind: "contact",
                args,
              }),
            )
            .then((result) => hideAndBadge(item, result, onHide))
            .catch((err) => console.error("Failed to open contact:", err));
        }
      }
      break;
    case "windowAction":
      ipc("launcher_window_action", { action: item.kind.action })
        .then(() =>
          ipc<LauncherExecuteResult>("launcher_execute", {
            itemId: item.id,
            kind: "window",
            args,
          }),
        )
        .then((result) => hideAndBadge(item, result, onHide))
        .catch((err) => console.error("Failed to apply window action:", err));
      break;
    case "brewPackage":
      navigator.clipboard
        .writeText(item.kind.name)
        .then(() => onHide())
        .catch((err) => console.error("Failed to copy package name:", err));
      break;
    default:
      // task, note, calendar — navigate to them in main window
      ipc("launcher_execute", { itemId: item.id, kind: item.kind.type, args })
        .then(() => onExpandToMain())
        .catch((err) => console.error("Failed to execute:", err));
      break;
  }
}
