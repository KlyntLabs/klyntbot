import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { AgentStatus, AppInfoResponse } from "@shared/types";
import { SaveButton, ShortcutRecorder } from "@shared/ui";
import { useMemo, useState } from "react";
import { PermissionsCard } from "../components/PermissionsCard";

const SHORTCUT_DEFAULTS = {
  launcher: "alt+space",
  tray: "alt+shift+space",
};

export function GeneralSettings() {
  const { data: appInfo } = useQuery<AppInfoResponse>("app_info", undefined, {
    version: "...",
    dataDir: "...",
    setupCompleted: false,
  });

  const { data: status } = useQuery<AgentStatus>("agent_status", undefined, {
    status: "unknown",
    activeTaskCount: 0,
    focusTask: null,
  });

  const { data: shortcutsConfig, refetch: refetchShortcuts } = useQuery<typeof SHORTCUT_DEFAULTS>(
    "shortcuts_get",
    undefined,
    SHORTCUT_DEFAULTS,
  );

  const [shortcutEdits, setShortcutEdits] = useState<Record<string, string>>({});
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [savingShortcuts, setSavingShortcuts] = useState(false);

  const currentShortcuts = useMemo(
    () => ({
      launcher: shortcutEdits.launcher ?? shortcutsConfig.launcher,
      tray: shortcutEdits.tray ?? shortcutsConfig.tray,
    }),
    [shortcutEdits, shortcutsConfig],
  );

  const hasShortcutChanges = Object.keys(shortcutEdits).length > 0;

  const duplicateShortcut = useMemo(() => {
    const values = Object.values(currentShortcuts);
    return values.find((v, i) => values.indexOf(v) !== i) ?? null;
  }, [currentShortcuts]);

  const handleSaveShortcuts = async () => {
    setSavingShortcuts(true);
    setShortcutError(null);
    try {
      await ipc("shortcuts_update", currentShortcuts);
      refetchShortcuts();
      setShortcutEdits({});
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setShortcutError(msg);
    } finally {
      setSavingShortcuts(false);
    }
  };

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">General</h2>
        <p className="text-[13px] text-muted-foreground mt-1">Overview and system information</p>
      </div>

      <div className="space-y-4">
        <SettingsCard title="System">
          <div className="space-y-2">
            <div className="flex justify-between text-[13px]">
              <span className="text-muted-foreground">Version</span>
              <span className="text-muted-foreground font-mono">{appInfo.version}</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted-foreground">Data directory</span>
              <span className="text-muted-foreground font-mono">{appInfo.dataDir}</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted-foreground">Agent status</span>
              <span className="text-muted-foreground">{status.status}</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted-foreground">Active tasks</span>
              <span className="text-muted-foreground">{status.activeTaskCount}</span>
            </div>
          </div>
        </SettingsCard>

        <SettingsCard title="Keyboard Shortcuts">
          <div className="space-y-3">
            {(
              [
                ["launcher", "Launcher"],
                ["tray", "Tray popup"],
              ] as const
            ).map(([key, label]) => (
              <div key={key} className="flex items-center justify-between gap-4">
                <span className="text-xs text-muted-foreground w-28 shrink-0">{label}</span>
                <ShortcutRecorder
                  value={currentShortcuts[key]}
                  defaultValue={SHORTCUT_DEFAULTS[key]}
                  onChange={(val) => setShortcutEdits((prev) => ({ ...prev, [key]: val }))}
                  error={
                    duplicateShortcut && currentShortcuts[key] === duplicateShortcut
                      ? "Duplicate shortcut"
                      : undefined
                  }
                />
              </div>
            ))}

            {shortcutError && <p className="text-xs text-red-400">{shortcutError}</p>}

            {hasShortcutChanges && (
              <div className="flex justify-end">
                <SaveButton
                  onClick={handleSaveShortcuts}
                  saving={savingShortcuts}
                  disabled={duplicateShortcut !== null}
                />
              </div>
            )}
          </div>
        </SettingsCard>

        <PermissionsCard />
      </div>
    </div>
  );
}
