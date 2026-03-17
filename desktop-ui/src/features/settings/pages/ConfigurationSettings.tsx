import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { SaveButton, SecretInput, Toggle } from "@shared/ui";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";

// ── Channel definitions ──────────────────────────────────────────────

interface ChannelField {
  key: string;
  label: string;
  secret?: boolean;
  placeholder?: string;
}

interface ChannelDef {
  key: string;
  label: string;
  fields: ChannelField[];
}

const CHANNEL_DEFS: ChannelDef[] = [
  {
    key: "telegram",
    label: "Telegram",
    fields: [{ key: "token", label: "Bot Token", secret: true, placeholder: "123456:ABC-DEF..." }],
  },
  {
    key: "discord",
    label: "Discord",
    fields: [{ key: "token", label: "Bot Token", secret: true, placeholder: "Discord bot token" }],
  },
  {
    key: "slack",
    label: "Slack",
    fields: [
      { key: "botToken", label: "Bot Token", secret: true, placeholder: "xoxb-..." },
      { key: "appToken", label: "App Token", secret: true, placeholder: "xapp-..." },
    ],
  },
  {
    key: "email",
    label: "Email (IMAP/SMTP)",
    fields: [
      { key: "imapHost", label: "IMAP Host", placeholder: "imap.gmail.com" },
      { key: "imapUsername", label: "IMAP Username", placeholder: "user@gmail.com" },
      { key: "imapPassword", label: "IMAP Password", secret: true },
      { key: "smtpHost", label: "SMTP Host", placeholder: "smtp.gmail.com" },
      { key: "smtpUsername", label: "SMTP Username", placeholder: "user@gmail.com" },
      { key: "smtpPassword", label: "SMTP Password", secret: true },
      { key: "fromAddress", label: "From Address", placeholder: "user@gmail.com" },
    ],
  },
];

// ── Types ────────────────────────────────────────────────────────────

interface ChannelsData {
  [key: string]: { enabled?: boolean; [field: string]: unknown };
}

interface ToolsData {
  restrictToWorkspace?: boolean;
  browser?: { enabled?: boolean; trustLevel?: string };
  web?: { braveApiKey?: string };
}

interface GatewayData {
  host?: string;
  port?: number;
}

// ── Component ────────────────────────────────────────────────────────

export function ConfigurationSettings() {
  const { data: channels, refetch: refetchChannels } = useQuery<ChannelsData>(
    "config_get_section",
    { section: "channels" },
    {},
  );

  const { data: tools, refetch: refetchTools } = useQuery<ToolsData>(
    "config_get_section",
    { section: "tools" },
    {},
  );

  const { data: gateway, refetch: refetchGateway } = useQuery<GatewayData>(
    "config_get_section",
    { section: "gateway" },
    { host: "127.0.0.1", port: 18790 },
  );

  const [expanded, setExpanded] = useState<string | null>(null);
  const [channelEdits, setChannelEdits] = useState<Record<string, Record<string, unknown>>>({});
  const [toolEdits, setToolEdits] = useState<Record<string, unknown>>({});
  const [gatewayEdits, setGatewayEdits] = useState<Record<string, unknown>>({});
  const [saving, setSaving] = useState<string | null>(null);

  // ── Channel helpers ──────────────────────────────────────────────

  const getChannelValue = (channelKey: string, field: string): unknown => {
    return (
      channelEdits[channelKey]?.[field] ??
      (channels[channelKey] as Record<string, unknown>)?.[field] ??
      ""
    );
  };

  const setChannelEdit = (channelKey: string, field: string, value: unknown) => {
    setChannelEdits((prev) => ({
      ...prev,
      [channelKey]: { ...prev[channelKey], [field]: value },
    }));
  };

  const saveChannel = async (channelKey: string) => {
    const edits = channelEdits[channelKey];
    if (!edits || Object.keys(edits).length === 0) return;
    setSaving(channelKey);
    try {
      await ipc("config_update_section", {
        section: "channels",
        patch: { [channelKey]: edits },
      });
      refetchChannels();
      setChannelEdits((prev) => {
        const next = { ...prev };
        delete next[channelKey];
        return next;
      });
    } catch (e) {
      console.error("Failed to save channel config:", e);
    } finally {
      setSaving(null);
    }
  };

  // ── Tools helpers ────────────────────────────────────────────────

  const getToolValue = (path: string): unknown => {
    if (path in toolEdits) return toolEdits[path];
    const parts = path.split(".");
    let val: unknown = tools;
    for (const p of parts) {
      if (val && typeof val === "object") val = (val as Record<string, unknown>)[p];
      else return "";
    }
    return val ?? "";
  };

  const saveTools = async () => {
    if (Object.keys(toolEdits).length === 0) return;
    setSaving("tools");
    try {
      // Build nested patch from flat keys
      const patch: Record<string, unknown> = {};
      for (const [path, value] of Object.entries(toolEdits)) {
        const parts = path.split(".");
        let target = patch;
        for (let i = 0; i < parts.length - 1; i++) {
          target[parts[i]] = target[parts[i]] || {};
          target = target[parts[i]] as Record<string, unknown>;
        }
        target[parts[parts.length - 1]] = value;
      }
      await ipc("config_update_section", { section: "tools", patch });
      refetchTools();
      setToolEdits({});
    } catch (e) {
      console.error("Failed to save tools config:", e);
    } finally {
      setSaving(null);
    }
  };

  // ── Gateway helpers ──────────────────────────────────────────────

  const saveGateway = async () => {
    if (Object.keys(gatewayEdits).length === 0) return;
    setSaving("gateway");
    try {
      await ipc("config_update_section", { section: "gateway", patch: gatewayEdits });
      refetchGateway();
      setGatewayEdits({});
    } catch (e) {
      console.error("Failed to save gateway config:", e);
    } finally {
      setSaving(null);
    }
  };

  const hasToolChanges = Object.keys(toolEdits).length > 0;
  const hasGatewayChanges = Object.keys(gatewayEdits).length > 0;

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Configuration</h2>
        <p className="text-[13px] text-muted-foreground mt-1">Channels, tools, and gateway settings</p>
      </div>

      <div className="space-y-4">
        {/* ── Channels ──────────────────────────────────────────── */}
        <SettingsCard title="Channels">
          <div className="space-y-1.5">
            {CHANNEL_DEFS.map((ch) => {
              const isExpanded = expanded === ch.key;
              const enabled = getChannelValue(ch.key, "enabled") as boolean;
              const hasEdits = Object.keys(channelEdits[ch.key] ?? {}).length > 0;

              return (
                <div
                  key={ch.key}
                  className="bg-card rounded-lg border border-border-subtle"
                >
                  <div className="flex items-center gap-2 p-3">
                    <button
                      type="button"
                      onClick={() => setExpanded(isExpanded ? null : ch.key)}
                      className="text-muted-foreground hover:text-foreground transition-colors"
                    >
                      {isExpanded ? (
                        <ChevronDown className="w-3.5 h-3.5" />
                      ) : (
                        <ChevronRight className="w-3.5 h-3.5" />
                      )}
                    </button>
                    <span className="flex-1 text-[13px] font-medium text-muted-foreground">
                      {ch.label}
                    </span>
                    <Toggle
                      checked={enabled}
                      onChange={(v) => setChannelEdit(ch.key, "enabled", v)}
                    />
                  </div>

                  {isExpanded && (
                    <div className="px-3 pb-3 space-y-2 border-t border-border-subtle pt-2">
                      {ch.fields.map((field) => (
                        <div key={field.key}>
                          <span className="block text-[11px] text-muted-foreground mb-1">{field.label}</span>
                          {field.secret ? (
                            <SecretInput
                              value={String(getChannelValue(ch.key, field.key) || "")}
                              onChange={(v) => setChannelEdit(ch.key, field.key, v)}
                              placeholder={field.placeholder}
                            />
                          ) : (
                            <input
                              type="text"
                              value={String(getChannelValue(ch.key, field.key) || "")}
                              onChange={(e) => setChannelEdit(ch.key, field.key, e.target.value)}
                              placeholder={field.placeholder}
                              className="w-full px-3 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                            />
                          )}
                        </div>
                      ))}
                      {hasEdits && (
                        <div className="pt-1">
                          <SaveButton
                            onClick={() => saveChannel(ch.key)}
                            saving={saving === ch.key}
                          />
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </SettingsCard>

        {/* ── Tools ─────────────────────────────────────────────── */}
        <SettingsCard title="Tools">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-[12px] text-muted-foreground">Restrict to workspace</span>
                <p className="text-[11px] text-dim">
                  Limit file operations to the workspace directory
                </p>
              </div>
              <Toggle
                checked={(getToolValue("restrictToWorkspace") as boolean) || false}
                onChange={(v) => setToolEdits((prev) => ({ ...prev, restrictToWorkspace: v }))}
              />
            </div>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-[12px] text-muted-foreground">Browser automation</span>
                <p className="text-[11px] text-dim">Enable the browser tool for web automation</p>
              </div>
              <Toggle
                checked={(getToolValue("browser.enabled") as boolean) || false}
                onChange={(v) => setToolEdits((prev) => ({ ...prev, "browser.enabled": v }))}
              />
            </div>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Browser trust level</span>
              <select
                value={String(getToolValue("browser.trustLevel") || "autonomous")}
                onChange={(e) =>
                  setToolEdits((prev) => ({ ...prev, "browser.trustLevel": e.target.value }))
                }
                className="w-full px-3 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                <option value="strict" className="bg-surface-floating">
                  Strict — confirm every write
                </option>
                <option value="autonomous" className="bg-surface-floating">
                  Autonomous — confirm dangerous only
                </option>
                <option value="full" className="bg-surface-floating">
                  Full — no confirmations
                </option>
              </select>
            </label>

            <div>
              <span className="block text-[11px] text-muted-foreground mb-1">Brave Search API key</span>
              <SecretInput
                value={String(getToolValue("web.braveApiKey") || "")}
                onChange={(v) => setToolEdits((prev) => ({ ...prev, "web.braveApiKey": v }))}
                placeholder="BSA-..."
              />
            </div>

            {hasToolChanges && <SaveButton onClick={saveTools} saving={saving === "tools"} />}
          </div>
        </SettingsCard>

        {/* ── Gateway ───────────────────────────────────────────── */}
        <SettingsCard title="Gateway">
          <div className="space-y-3">
            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Host</span>
                <input
                  type="text"
                  value={String(
                    "host" in gatewayEdits ? gatewayEdits.host : (gateway.host ?? "127.0.0.1"),
                  )}
                  onChange={(e) => setGatewayEdits((prev) => ({ ...prev, host: e.target.value }))}
                  className="w-full px-3 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
              <label className="w-28">
                <span className="block text-[11px] text-muted-foreground mb-1">Port</span>
                <input
                  type="number"
                  value={
                    "port" in gatewayEdits ? (gatewayEdits.port as number) : (gateway.port ?? 18790)
                  }
                  onChange={(e) =>
                    setGatewayEdits((prev) => ({
                      ...prev,
                      port: Number.parseInt(e.target.value, 10) || 0,
                    }))
                  }
                  className="w-full px-3 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
            </div>

            {hasGatewayChanges && (
              <SaveButton onClick={saveGateway} saving={saving === "gateway"} />
            )}
          </div>
        </SettingsCard>
      </div>
    </div>
  );
}
