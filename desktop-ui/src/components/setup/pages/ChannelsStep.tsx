import { Eye, EyeOff } from "lucide-react";
import { useState } from "react";
import { useOutletContext } from "react-router";
import { ipc } from "../../../hooks/useIpc";
import type { SetupContext } from "../steps";

interface ChannelDef {
  key: string;
  label: string;
  tokenLabel: string;
  tokenPlaceholder: string;
}

const CHANNELS: ChannelDef[] = [
  {
    key: "telegram",
    label: "Telegram",
    tokenLabel: "Bot Token",
    tokenPlaceholder: "123456:ABC-DEF...",
  },
  {
    key: "discord",
    label: "Discord",
    tokenLabel: "Bot Token",
    tokenPlaceholder: "Discord bot token",
  },
  {
    key: "slack",
    label: "Slack",
    tokenLabel: "Bot Token",
    tokenPlaceholder: "xoxb-...",
  },
  {
    key: "whatsapp",
    label: "WhatsApp",
    tokenLabel: "Access Token",
    tokenPlaceholder: "WhatsApp Cloud API token",
  },
  {
    key: "email",
    label: "Email (IMAP/SMTP)",
    tokenLabel: "IMAP Host",
    tokenPlaceholder: "imap.gmail.com",
  },
];

interface ChannelState {
  enabled: boolean;
  token: string;
  showToken: boolean;
}

export function ChannelsStep() {
  const { next } = useOutletContext<SetupContext>();
  const [saving, setSaving] = useState(false);

  const [channels, setChannels] = useState<Record<string, ChannelState>>(() =>
    Object.fromEntries(
      CHANNELS.map((ch) => [ch.key, { enabled: false, token: "", showToken: false }]),
    ),
  );

  const updateChannel = (key: string, update: Partial<ChannelState>) => {
    setChannels((prev) => ({
      ...prev,
      [key]: { ...prev[key], ...update },
    }));
  };

  const handleContinue = async () => {
    const patch: Record<string, unknown> = {};
    for (const ch of CHANNELS) {
      const state = channels[ch.key];
      if (state.enabled && state.token.trim()) {
        patch[ch.key] = { enabled: true, token: state.token.trim() };
      }
    }

    if (Object.keys(patch).length > 0) {
      setSaving(true);
      try {
        await ipc("config_update_section", { section: "channels", patch });
      } catch {
        // Non-blocking — user can configure later
      } finally {
        setSaving(false);
      }
    }

    next();
  };

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-1">Channels</h2>
      <p className="text-[13px] text-muted mb-6">
        Connect chat platforms so Klynt can respond on your behalf. All channels are optional.
      </p>

      <div className="space-y-3 max-h-[320px] overflow-y-auto pr-1">
        {CHANNELS.map((ch) => {
          const state = channels[ch.key];
          return (
            <div key={ch.key} className="bg-surface-lowest rounded-lg border border-border p-3">
              <div className="flex items-center justify-between mb-2">
                <span className="text-[13px] font-medium text-secondary">{ch.label}</span>
                <button
                  type="button"
                  onClick={() => updateChannel(ch.key, { enabled: !state.enabled })}
                  className={`relative w-9 h-5 rounded-full transition-colors ${
                    state.enabled ? "bg-brand" : "bg-white/[0.1]"
                  }`}
                >
                  <span
                    className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                      state.enabled ? "translate-x-4" : ""
                    }`}
                  />
                </button>
              </div>

              {state.enabled && (
                <label className="block">
                  <span className="block text-[11px] text-muted mb-1">{ch.tokenLabel}</span>
                  <div className="relative">
                    <input
                      type={state.showToken ? "text" : "password"}
                      value={state.token}
                      onChange={(e) => updateChannel(ch.key, { token: e.target.value })}
                      placeholder={ch.tokenPlaceholder}
                      className="w-full px-3 py-1.5 pr-9 text-[12px] text-primary bg-surface-base border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                    />
                    <button
                      type="button"
                      onClick={() => updateChannel(ch.key, { showToken: !state.showToken })}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-muted hover:text-secondary transition-colors"
                    >
                      {state.showToken ? (
                        <EyeOff className="w-3 h-3" />
                      ) : (
                        <Eye className="w-3 h-3" />
                      )}
                    </button>
                  </div>
                </label>
              )}
            </div>
          );
        })}
      </div>

      <div className="mt-6 flex justify-end">
        <button
          type="button"
          onClick={handleContinue}
          disabled={saving}
          className="px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors disabled:opacity-50"
        >
          {saving ? "Saving..." : "Continue"}
        </button>
      </div>
    </div>
  );
}
