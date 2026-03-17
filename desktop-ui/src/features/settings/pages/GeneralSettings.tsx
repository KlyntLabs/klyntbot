import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { AgentStatus, AppInfoResponse } from "@shared/types";
import { useState } from "react";
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

  const { data: agentsConfig, refetch } = useQuery<AgentsConfig>(
    "config_get_section",
    { section: "agents" },
    { defaults: {} },
  );

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
    } catch (e) {
      console.error("Failed to save agent defaults:", e);
    } finally {
      setSaving(false);
    }
  };

  const hasChanges = model !== null || temperature !== null || maxTokens !== null;

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">General</h2>
        <p className="text-[13px] text-muted mt-1">Overview and system information</p>
      </div>

      <div className="space-y-4">
        <div className="bg-surface-low rounded-lg border border-border p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">System</h3>
          <div className="space-y-2">
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Version</span>
              <span className="text-secondary font-mono">{appInfo.version}</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Data directory</span>
              <span className="text-secondary font-mono">{appInfo.dataDir}</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Agent status</span>
              <span className="text-secondary">{status.status}</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Active tasks</span>
              <span className="text-secondary">{status.activeTaskCount}</span>
            </div>
          </div>
        </div>

        <div className="bg-surface-low rounded-lg border border-border p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">Agent defaults</h3>
          <div className="space-y-3">
            <label className="block">
              <span className="block text-[12px] text-muted mb-1">Default model</span>
              <input
                type="text"
                value={currentModel}
                onChange={(e) => setModel(e.target.value)}
                placeholder="e.g. anthropic/claude-opus-4-5"
                className="w-full px-3 py-1.5 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[12px] text-muted mb-1">Temperature</span>
                <input
                  type="number"
                  value={currentTemp}
                  onChange={(e) => setTemperature(e.target.value)}
                  step="0.1"
                  min="0"
                  max="2"
                  className="w-full px-3 py-1.5 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
              <label className="flex-1">
                <span className="block text-[12px] text-muted mb-1">Max tokens</span>
                <input
                  type="number"
                  value={currentMaxTokens}
                  onChange={(e) => setMaxTokens(e.target.value)}
                  step="256"
                  min="256"
                  className="w-full px-3 py-1.5 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
            </div>

            {hasChanges && (
              <div className="flex justify-end">
                <button
                  type="button"
                  onClick={handleSaveDefaults}
                  disabled={saving}
                  className="px-4 py-1.5 text-[12px] font-medium text-white bg-brand hover:bg-brand-hover rounded-lg transition-colors disabled:opacity-50"
                >
                  {saving ? "Saving..." : "Save changes"}
                </button>
              </div>
            )}
          </div>
        </div>

        <PermissionsCard />
      </div>
    </div>
  );
}
