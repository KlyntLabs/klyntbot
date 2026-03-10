import { useEffect, useState } from "react";
import { useOutletContext } from "react-router";
import { ipc } from "@shared/hooks/useIpc";
import { SecretInput, Toggle } from "@shared/ui";
import type { SetupContext } from "../hooks/steps";

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
    key: "email",
    label: "Email (IMAP/SMTP)",
    tokenLabel: "IMAP Host",
    tokenPlaceholder: "imap.gmail.com",
  },
];

interface ChannelState {
  enabled: boolean;
  token: string;
}

export function ChannelsStep() {
  const { forwardRef, setDirty } = useOutletContext<SetupContext>();

  const [channels, setChannels] = useState<Record<string, ChannelState>>(() =>
    Object.fromEntries(CHANNELS.map((ch) => [ch.key, { enabled: false, token: "" }])),
  );

  // Load saved channel config on mount
  useEffect(() => {
    ipc<Record<string, { enabled?: boolean; token?: string }>>("config_get_section", {
      section: "channels",
    })
      .then((saved) => {
        if (!saved || typeof saved !== "object") return;
        const loaded: Record<string, ChannelState> = {};
        let hasSaved = false;
        for (const ch of CHANNELS) {
          const cfg = saved[ch.key];
          if (cfg?.enabled) {
            loaded[ch.key] = { enabled: true, token: cfg.token ?? "" };
            hasSaved = true;
          } else {
            loaded[ch.key] = { enabled: false, token: "" };
          }
        }
        if (hasSaved) {
          setChannels(loaded);
          setDirty(true);
        }
      })
      .catch(() => {});
  }, [setDirty]);

  // Register save handler with layout
  useEffect(() => {
    forwardRef.current = async (isSkip: boolean) => {
      if (isSkip) return true;
      const patch: Record<string, unknown> = {};
      for (const ch of CHANNELS) {
        const state = channels[ch.key];
        if (state.enabled && state.token.trim()) {
          patch[ch.key] = { enabled: true, token: state.token.trim() };
        }
      }
      if (Object.keys(patch).length > 0) {
        await ipc("config_update_section", { section: "channels", patch });
      }
      return true;
    };
  }, [forwardRef, channels]);

  const updateChannel = (key: string, update: Partial<ChannelState>) => {
    setChannels((prev) => ({
      ...prev,
      [key]: { ...prev[key], ...update },
    }));
    if ("enabled" in update) setDirty(true);
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
                <Toggle
                  checked={state.enabled}
                  onChange={(v) => updateChannel(ch.key, { enabled: v })}
                />
              </div>

              {state.enabled && (
                <label className="block">
                  <span className="block text-[11px] text-muted mb-1">{ch.tokenLabel}</span>
                  <SecretInput
                    value={state.token}
                    onChange={(v) => {
                      updateChannel(ch.key, { token: v });
                      setDirty(true);
                    }}
                    placeholder={ch.tokenPlaceholder}
                  />
                </label>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
