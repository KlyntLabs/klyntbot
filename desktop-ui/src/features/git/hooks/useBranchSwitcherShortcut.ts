import { useCallback } from "react";
import { useGlobalShortcut } from "@/hooks/useGlobalShortcut";

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
  const handler = useCallback(
    (event: KeyboardEvent) => {
      event.preventDefault();
      onTrigger();
    },
    [onTrigger],
  );

  useGlobalShortcut({
    shortcuts: [{ shortcut, handler }],
    enabled: isEnabled,
  });
}
