import { useState } from "react";
import { useFocusTimer } from "../hooks/useFocusTimer";
import { FocusSettingsPanel } from "./FocusSettingsPanel";
import { FocusTimer } from "./FocusTimer";
import "../focus.css";

export function FocusPage() {
  const timer = useFocusTimer();
  const [settingsOpen, setSettingsOpen] = useState(false);

  return (
    <div className="tc-page">
      <FocusTimer timer={timer} onOpenSettings={() => setSettingsOpen(true)} />
      {settingsOpen && (
        <FocusSettingsPanel
          settings={timer.settings}
          onUpdate={timer.updateSettings}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}
