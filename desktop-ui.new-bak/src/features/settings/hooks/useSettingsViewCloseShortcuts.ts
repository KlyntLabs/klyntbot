import { useCallback } from "react";
import { useGlobalShortcut } from "@/hooks/useGlobalShortcut";

export const useSettingsViewCloseShortcuts = (onClose: () => void) => {
  const handleEscape = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    },
    [onClose],
  );

  const handleCloseShortcut = useCallback(
    (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "w") {
        event.preventDefault();
        onClose();
      }
    },
    [onClose],
  );

  useGlobalShortcut({
    shortcuts: [
      { shortcut: "escape", handler: handleEscape, allowInInput: true },
      { shortcut: "cmd+w", handler: handleCloseShortcut },
    ],
  });
};
