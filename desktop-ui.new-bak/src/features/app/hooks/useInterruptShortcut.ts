import { useCallback } from "react";
import { useGlobalShortcut } from "@/hooks/useGlobalShortcut";

type UseInterruptShortcutOptions = {
  isEnabled: boolean;
  shortcut: string | null;
  onTrigger: () => void | Promise<void>;
};

export function useInterruptShortcut({
  isEnabled,
  shortcut,
  onTrigger,
}: UseInterruptShortcutOptions) {
  const handler = useCallback(
    (event: KeyboardEvent) => {
      event.preventDefault();
      void onTrigger();
    },
    [onTrigger],
  );

  useGlobalShortcut({
    shortcuts: [{ shortcut, handler }],
    enabled: isEnabled,
  });
}
