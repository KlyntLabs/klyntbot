import { isMacPlatform } from "@utils/shortcuts";
import { useMemo } from "react";
import { useGlobalShortcut } from "@/hooks/useGlobalShortcut";

type UseNewAgentShortcutOptions = {
  isEnabled: boolean;
  onTrigger: () => void;
};

export function useNewAgentShortcut({ isEnabled, onTrigger }: UseNewAgentShortcutOptions) {
  const shortcut = useMemo(() => (isMacPlatform() ? "cmd+n" : "ctrl+n"), []);

  useGlobalShortcut({
    shortcuts: [{ shortcut, handler: onTrigger }],
    enabled: isEnabled,
  });
}
