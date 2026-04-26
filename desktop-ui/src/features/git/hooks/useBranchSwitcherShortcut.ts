import { matchesShortcut } from "@utils/shortcuts";
import { useEffect } from "react";

type UseBranchSwitcherShortcutOptions = {
  shortcut: string | null;
  isEnabled: boolean;
  onTrigger: () => void;
};

export function useBranchSwitcherShortcut({
  shortcut,
  isEnabled,
  onTrigger,
}: UseBranchSwitcherShortcutOptions) {
  useEffect(() => {
    if (!isEnabled || !shortcut) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (matchesShortcut(event, shortcut)) {
        event.preventDefault();
        onTrigger();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isEnabled, onTrigger, shortcut]);
}
