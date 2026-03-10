import { CheckCircle } from "lucide-react";
import { useNavigate } from "react-router";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { McpConfigResponse } from "@shared/types";

interface ConfigSummary {
  provider?: string;
  model?: string;
  channels: number;
  accounts: number;
  mcpServers: number;
}

function useSummary(): ConfigSummary {
  const { data: agents } = useQuery<{ defaults?: { provider?: string; model?: string } }>(
    "config_get_section",
    { section: "agents" },
    { defaults: {} },
  );

  const { data: channels } = useQuery<Record<string, { enabled?: boolean }>>(
    "config_get_section",
    { section: "channels" },
    {},
  );

  const { data: accountsList } = useQuery<{ id: string }[]>("finance_accounts", undefined, []);

  const { data: mcpConfig } = useQuery<McpConfigResponse>("mcp_get_config", undefined, {
    enabled: true,
    servers: [],
  });

  const enabledChannels = Object.values(channels).filter(
    (ch) => ch && typeof ch === "object" && ch.enabled,
  ).length;

  return {
    provider: agents.defaults?.provider,
    model: agents.defaults?.model,
    channels: enabledChannels,
    accounts: accountsList.length,
    mcpServers: mcpConfig.servers.length,
  };
}

export function CompleteStep() {
  const navigate = useNavigate();
  const summary = useSummary();

  const handleLaunch = async () => {
    await ipc("config_mark_setup_completed").catch(() => {});
    navigate("/");
  };

  const items = [
    summary.provider && { label: "Provider", value: summary.provider },
    summary.model && { label: "Model", value: summary.model },
    summary.channels > 0 && {
      label: "Channels",
      value: `${summary.channels} connected`,
    },
    summary.accounts > 0 && {
      label: "Accounts",
      value: `${summary.accounts} added`,
    },
    summary.mcpServers > 0 && {
      label: "MCP servers",
      value: `${summary.mcpServers} installed`,
    },
  ].filter(Boolean) as { label: string; value: string }[];

  return (
    <div className="text-center py-4">
      <CheckCircle className="w-12 h-12 text-success mx-auto mb-4" strokeWidth={1.5} />
      <h2 className="text-lg font-medium text-primary mb-2">You're all set!</h2>
      <p className="text-[13px] text-muted mb-6">
        Klynt is configured and ready to go. You can always change these settings later.
      </p>

      {items.length > 0 && (
        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] p-4 mb-6 text-left max-w-xs mx-auto">
          <h3 className="text-[11px] font-medium text-dim uppercase tracking-wider mb-2">
            Configuration summary
          </h3>
          <div className="space-y-1.5">
            {items.map((item) => (
              <div key={item.label} className="flex justify-between text-[12px]">
                <span className="text-muted">{item.label}</span>
                <span className="text-secondary font-mono">{item.value}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <button
        type="button"
        onClick={handleLaunch}
        className="px-6 py-2.5 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
      >
        Launch Klynt
      </button>
    </div>
  );
}
