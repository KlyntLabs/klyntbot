import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useEffect, useRef, useState } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem, LauncherItemKind } from "../types";

interface Action {
  label: string;
  shortcut?: string;
  handler: () => void;
}

export function ActionMenu() {
  const actionMenuOpen = useLauncherStore((s) => s.actionMenuOpen);
  const results = useLauncherStore((s) => s.results);
  const selectedIndex = useLauncherStore((s) => s.selectedIndex);
  const [focusedIndex, setFocusedIndex] = useState(0);
  const menuRef = useRef<HTMLDivElement>(null);

  const item = results[selectedIndex] ?? null;
  const actions = item ? getActionsForItem(item) : [];

  const close = useCallback(() => {
    useLauncherStore.getState().setActionMenuOpen(false);
  }, []);

  const executeAction = useCallback(
    (index: number) => {
      const action = actions[index];
      if (action) {
        action.handler();
        close();
      }
    },
    [actions, close],
  );

  // Reset focus when menu opens
  useEffect(() => {
    if (actionMenuOpen) {
      setFocusedIndex(0);
    }
  }, [actionMenuOpen]);

  // Keyboard handling within the action menu
  useEffect(() => {
    if (!actionMenuOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case "Escape": {
          e.preventDefault();
          e.stopPropagation();
          close();
          break;
        }
        case "ArrowDown": {
          e.preventDefault();
          e.stopPropagation();
          setFocusedIndex((prev) => Math.min(prev + 1, actions.length - 1));
          break;
        }
        case "ArrowUp": {
          e.preventDefault();
          e.stopPropagation();
          setFocusedIndex((prev) => Math.max(prev - 1, 0));
          break;
        }
        case "Enter": {
          e.preventDefault();
          e.stopPropagation();
          executeAction(focusedIndex);
          break;
        }
        default: {
          // Quick select with 1-9
          const num = Number.parseInt(e.key, 10);
          if (num >= 1 && num <= 9 && num <= actions.length) {
            e.preventDefault();
            e.stopPropagation();
            executeAction(num - 1);
          }
          break;
        }
      }
    };

    // Use capture phase to intercept before the main keyboard handler
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [actionMenuOpen, actions.length, close, executeAction, focusedIndex]);

  if (!actionMenuOpen || !item) return null;

  return (
    <div
      role="dialog"
      className="absolute inset-0 z-50 flex items-center justify-center"
      onClick={close}
      onKeyDown={() => {}}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-overlay" />

      {/* Menu */}
      <div
        ref={menuRef}
        role="menu"
        className="relative glass-panel rounded-lg w-[320px] max-h-[400px] overflow-hidden animate-[result-in_0.15s_ease-out_both]"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={() => {}}
      >
        {/* Header */}
        <div className="px-4 py-2.5 border-b border-border">
          <div className="text-xs text-muted uppercase tracking-wider mb-0.5">Actions</div>
          <div className="text-sm text-foreground truncate">{item.title}</div>
        </div>

        {/* Action list */}
        <div className="py-1 overflow-y-auto max-h-[320px]">
          {actions.map((action, index) => (
            <button
              key={action.label}
              type="button"
              className={`w-full flex items-center gap-3 px-4 py-2 text-left transition-colors duration-100 ${
                index === focusedIndex ? "bg-surface-raised" : "hover:bg-surface-raised/50"
              }`}
              onClick={() => executeAction(index)}
              onMouseEnter={() => setFocusedIndex(index)}
            >
              <span className="text-xs text-muted w-4 text-center shrink-0">{index + 1}</span>
              <span className="text-sm text-foreground flex-1">{action.label}</span>
              {action.shortcut && <span className="text-[10px] text-muted">{action.shortcut}</span>}
            </button>
          ))}
        </div>

        {/* Footer hint */}
        <div className="px-4 py-1.5 border-t border-border text-[10px] text-muted">
          1-9 quick select &middot; &uarr;&darr; navigate &middot; Esc close
        </div>
      </div>
    </div>
  );
}

function copyText(text: string) {
  navigator.clipboard.writeText(text).catch((err) => console.error("Failed to copy:", err));
}

function openPath(path: string) {
  ipc("launcher_open_app", { path }).catch((err) => console.error("Failed to open:", err));
}

function getActionsForItem(item: LauncherItem): Action[] {
  const { kind } = item;

  switch (kind.type) {
    case "application":
      return applicationActions(item, kind);
    case "file":
      return fileActions(item, kind);
    case "contentMatch":
      return contentMatchActions(item, kind);
    case "gitRepo":
      return gitRepoActions(item, kind);
    case "bookmark":
      return urlActions(item, kind, "bookmark");
    case "browserHistory":
      return urlActions(item, kind, "browserHistory");
    case "urlNavigation":
      return urlActions(item, kind, "urlNavigation");
    case "task":
      return taskActions(item);
    case "note":
      return noteActions(item);
    case "calculator":
      return calculatorActions(kind);
    case "contact":
      return contactActions(kind);
    case "systemCommand":
    case "script":
      return executeActions(item);
    case "sshHost":
      return sshActions(kind);
    case "brewPackage":
      return brewActions(kind);
    default:
      return defaultActions(item);
  }
}

type KindOf<T extends LauncherItemKind["type"]> = Extract<LauncherItemKind, { type: T }>;

function applicationActions(item: LauncherItem, kind: KindOf<"application">): Action[] {
  return [
    { label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
    { label: "Show in Finder", handler: () => openPath(kind.path.replace(/\/[^/]+$/, "")) },
    { label: "Copy Path", handler: () => copyText(kind.path) },
    { label: "Copy Name", handler: () => copyText(item.title) },
  ];
}

function fileActions(_item: LauncherItem, kind: KindOf<"file">): Action[] {
  const actions: Action[] = [
    { label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
    { label: "Reveal in Finder", handler: () => openPath(kind.path.replace(/\/[^/]+$/, "")) },
    { label: "Copy Path", handler: () => copyText(kind.path) },
  ];
  if (kind.kind === "folder") {
    actions.push({ label: "Open in Terminal", handler: () => openPath(`terminal://${kind.path}`) });
  }
  return actions;
}

function contentMatchActions(_item: LauncherItem, kind: KindOf<"contentMatch">): Action[] {
  return [
    { label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
    { label: "Reveal in Finder", handler: () => openPath(kind.path.replace(/\/[^/]+$/, "")) },
    { label: "Copy Path", handler: () => copyText(kind.path) },
  ];
}

function gitRepoActions(_item: LauncherItem, kind: KindOf<"gitRepo">): Action[] {
  return [
    { label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
    { label: "Reveal in Finder", handler: () => openPath(kind.path) },
    { label: "Copy Path", handler: () => copyText(kind.path) },
    { label: "Open in Terminal", handler: () => openPath(`terminal://${kind.path}`) },
  ];
}

function urlActions(_item: LauncherItem, kind: { url: string }, _type: string): Action[] {
  return [
    { label: "Open URL", shortcut: "Enter", handler: () => openPath(kind.url) },
    { label: "Copy URL", handler: () => copyText(kind.url) },
  ];
}

function taskActions(item: LauncherItem): Action[] {
  return [
    {
      label: "Open in App",
      shortcut: "Enter",
      handler: () => openPath(`klyntbot://task/${item.id}`),
    },
    { label: "Copy Title", handler: () => copyText(item.title) },
  ];
}

function noteActions(item: LauncherItem): Action[] {
  return [
    {
      label: "Open in App",
      shortcut: "Enter",
      handler: () => openPath(`klyntbot://note/${item.id}`),
    },
    { label: "Copy Title", handler: () => copyText(item.title) },
  ];
}

function calculatorActions(kind: KindOf<"calculator">): Action[] {
  return [
    { label: "Copy Result", shortcut: "Enter", handler: () => copyText(String(kind.result)) },
    { label: "Copy Expression", handler: () => copyText(kind.expression) },
  ];
}

function contactActions(kind: KindOf<"contact">): Action[] {
  const actions: Action[] = [];
  const { email, phone } = kind;
  if (email) {
    actions.push({
      label: "Email",
      shortcut: "Enter",
      handler: () => openPath(`mailto:${email}`),
    });
  }
  if (phone) {
    actions.push({ label: "Call", handler: () => openPath(`tel:${phone}`) });
  }
  if (email) {
    actions.push({ label: "Copy Email", handler: () => copyText(email) });
  }
  if (phone) {
    actions.push({ label: "Copy Phone", handler: () => copyText(phone) });
  }
  return actions;
}

function executeActions(item: LauncherItem): Action[] {
  return [
    { label: "Execute", shortcut: "Enter", handler: () => {} },
    { label: "Copy Title", handler: () => copyText(item.title) },
  ];
}

function sshActions(kind: KindOf<"sshHost">): Action[] {
  const sshCmd = kind.user ? `ssh ${kind.user}@${kind.host}` : `ssh ${kind.host}`;
  const sshUri = kind.user ? `ssh://${kind.user}@${kind.host}` : `ssh://${kind.host}`;
  return [
    { label: "Connect", shortcut: "Enter", handler: () => openPath(sshUri) },
    { label: "Copy SSH Command", handler: () => copyText(sshCmd) },
  ];
}

function brewActions(kind: KindOf<"brewPackage">): Action[] {
  const installCmd = kind.isCask ? `brew install --cask ${kind.name}` : `brew install ${kind.name}`;
  return [
    { label: "Copy Name", shortcut: "Enter", handler: () => copyText(kind.name) },
    { label: "Copy Install Command", handler: () => copyText(installCmd) },
  ];
}

function defaultActions(item: LauncherItem): Action[] {
  return [
    { label: "Execute", shortcut: "Enter", handler: () => {} },
    { label: "Copy Title", handler: () => copyText(item.title) },
  ];
}
