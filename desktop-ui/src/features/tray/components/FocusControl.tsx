import { useState } from "react";
import { FocusTimer } from "../../focus/components/FocusTimer";
import { FocusSettingsPanel } from "../../focus/components/FocusSettingsPanel";
import { type useFocusTimer } from "../../focus/hooks/useFocusTimer";

export function FocusControl({ timer }: { timer: ReturnType<typeof useFocusTimer> }) {
  const [view, setView] = useState<"timer" | "settings">("timer");
  if (view === "settings") {
    return (
      <FocusSettingsPanel
        settings={timer.settings}
        onUpdate={timer.updateSettings}
        onClose={() => setView("timer")}
      />
    );
  }
  return <FocusTimer timer={timer} onOpenSettings={() => setView("settings")} />;
}
