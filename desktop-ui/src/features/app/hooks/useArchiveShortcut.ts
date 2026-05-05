import { useCallback } from "react";
import { useGlobalShortcut } from "@/hooks/useGlobalShortcut";

type UseArchiveShortcutOptions = {
  isEnabled: boolean;
  shortcut: string | null;
  onTrigger: () => void;
};

export function useArchiveShortcut({ isEnabled, shortcut, onTrigger }: UseArchiveShortcutOptions) {
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
