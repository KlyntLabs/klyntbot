import { AutoTunerPanel } from "@features/autotuner";
import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import type { AgentStatus, AppInfoResponse } from "@shared/types";
import { SaveButton, ShortcutRecorder } from "@shared/ui";
import { useMemo, useState } from "react";
import { PermissionsCard } from "../components/PermissionsCard";

interface AgentDefaults {
  model?: string;
  provider?: string;
  temperature?: number;
  maxTokens?: number;
}

interface AgentsConfig {
  defaults?: AgentDefaults;
}

const SHORTCUT_DEFAULTS = {
  launcher: "alt+space",
  tray: "alt+shift+space",
  quickCapture: "super+shift+c",
};

export function GeneralSettings() {
  const toast = useToastContext();
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

  const { data: agentsConfig, refetch } = useQuery<AgentsConfig>(
    "config_get_section",
    { section: "agents" },
    { defaults: {} },
  );

  // ── Shortcuts ─────────────────────────────────────

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
      quickCapture: shortcutEdits.quickCapture ?? shortcutsConfig.quickCapture,
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

  // ── Agent defaults ────────────────────────────────

  const defaults = agentsConfig.defaults ?? {};
  const [model, setModel] = useState<string | null>(null);
  const [temperature, setTemperature] = useState<string | null>(null);
  const [maxTokens, setMaxTokens] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const currentModel = model ?? defaults.model ?? "";
  const currentTemp = temperature ?? String(defaults.temperature ?? 0.7);
  const currentMaxTokens = maxTokens ?? String(defaults.maxTokens ?? 8192);

  const handleSaveDefaults = async () => {
    setSaving(true);
    try {
      const patch: Record<string, unknown> = {
        defaults: {
          ...(model !== null && { model }),
          ...(temperature !== null && { temperature: Number.parseFloat(temperature) }),
          ...(maxTokens !== null && { maxTokens: Number.parseInt(maxTokens, 10) }),
        },
      };
      await ipc("config_update_section", { section: "agents", patch });
      refetch();
      setModel(null);
      setTemperature(null);
      setMaxTokens(null);
    } catch {
      toast.show("Failed to save agent defaults");
    } finally {
      setSaving(false);
    }
  };

  const hasChanges = model !== null || temperature !== null || maxTokens !== null;

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
                ["quickCapture", "Quick capture"],
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

        <SettingsCard title="Agent defaults">
          <div className="space-y-3">
            <label className="block">
              <span className="block text-xs text-muted-foreground mb-1">Default model</span>
              <input
                type="text"
                value={currentModel}
                onChange={(e) => setModel(e.target.value)}
                placeholder="e.g. anthropic/claude-opus-4-5"
                className="w-full px-3 py-1.5 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-xs text-muted-foreground mb-1">Temperature</span>
                <input
                  type="number"
                  value={currentTemp}
                  onChange={(e) => setTemperature(e.target.value)}
                  step="0.1"
                  min="0"
                  max="2"
                  className="w-full px-3 py-1.5 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
              <label className="flex-1">
                <span className="block text-xs text-muted-foreground mb-1">Max tokens</span>
                <input
                  type="number"
                  value={currentMaxTokens}
                  onChange={(e) => setMaxTokens(e.target.value)}
                  step="256"
                  min="256"
                  className="w-full px-3 py-1.5 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
            </div>

            {hasChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={handleSaveDefaults} saving={saving} />
              </div>
            )}
          </div>
        </SettingsCard>

        <PermissionsCard />

        <SettingsCard title="AI Self-Improvement">
          <p className="text-xs text-muted-foreground mb-3">
            AutoTuner continuously learns your preferences and optimizes response quality.
          </p>
          <AutoTunerPanel />
        </SettingsCard>
      </div>
    </div>
  );
}
