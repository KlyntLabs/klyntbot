import { useCallback } from "react";
import { useGlobalShortcut } from "@/hooks/useGlobalShortcut";

type UsePanelShortcutsOptions = {
  toggleDebugPanelShortcut: string | null;
  toggleTerminalShortcut: string | null;
  onToggleDebug: () => void;
  onToggleTerminal: () => void;
};

export function usePanelShortcuts({
  toggleDebugPanelShortcut,
  toggleTerminalShortcut,
  onToggleDebug,
  onToggleTerminal,
}: UsePanelShortcutsOptions) {
  const handleDebug = useCallback(
    (event: KeyboardEvent) => {
      event.preventDefault();
      onToggleDebug();
    },
    [onToggleDebug],
  );

  const handleTerminal = useCallback(
    (event: KeyboardEvent) => {
      event.preventDefault();
      onToggleTerminal();
    },
    [onToggleTerminal],
  );

  useGlobalShortcut({
    shortcuts: [
      { shortcut: toggleDebugPanelShortcut, handler: handleDebug },
      { shortcut: toggleTerminalShortcut, handler: handleTerminal },
    ],
  });
}
