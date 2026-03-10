import { recommendedServers } from "@features/settings/components/mcp/recommendedServers";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { McpAddServerParams, McpConfigResponse, RecommendedMcpServer } from "@shared/types";
import { Check, Download } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useOutletContext } from "react-router";
import type { SetupContext } from "../hooks/steps";

export function McpStep() {
  const { forwardRef, setDirty } = useOutletContext<SetupContext>();

  const { data: config, refetch } = useQuery<McpConfigResponse>("mcp_get_config", undefined, {
    enabled: true,
    servers: [],
  });

  const { mutate: addServer } = useMutation<McpConfigResponse, McpAddServerParams>(
    "mcp_add_server",
    "params",
  );

  const [installing, setInstalling] = useState<string | null>(null);

  const installedNames = useMemo(
    () => new Set(config.servers.map((s) => s.name)),
    [config.servers],
  );

  // Register forward handler — no save needed, installs are immediate
  useEffect(() => {
    forwardRef.current = async () => true;
  }, [forwardRef]);

  // Mark dirty when servers are installed (so button shows "Continue" instead of "Skip")
  useEffect(() => {
    if (config.servers.length > 0) setDirty(true);
  }, [config.servers.length, setDirty]);

  const handleInstall = useCallback(
    async (server: RecommendedMcpServer) => {
      if (!server.command) return;
      setInstalling(server.name);
      try {
        await addServer({
          name: server.name,
          transport: server.transport,
          command: server.command,
          args: server.args,
          env: {},
        });
        refetch();
        setDirty(true);
      } catch (e) {
        console.error("Failed to install MCP server:", e);
      } finally {
        setInstalling(null);
      }
    },
    [addServer, refetch, setDirty],
  );

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-1">MCP Servers</h2>
      <p className="text-[13px] text-muted mb-5">
        Connect external tools via the Model Context Protocol. Install recommended servers with one
        click — you can configure credentials and add custom servers later in Settings.
      </p>

      <div className="space-y-1.5 max-h-[320px] overflow-y-auto pr-1">
        {recommendedServers.map((server) => {
          const installed = installedNames.has(server.name);
          const isInstalling = installing === server.name;

          return (
            <div
              key={server.name}
              className="flex items-center gap-3 bg-white/[0.03] rounded-lg border border-white/[0.06] p-3"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-medium text-secondary">{server.name}</span>
                  <span className="text-[10px] text-dim">by {server.author}</span>
                </div>
                <p className="text-[11px] text-muted mt-0.5 truncate">{server.description}</p>
              </div>

              {installed ? (
                <span className="flex items-center gap-1 text-[11px] text-success">
                  <Check className="w-3.5 h-3.5" />
                  Installed
                </span>
              ) : (
                <button
                  type="button"
                  onClick={() => handleInstall(server)}
                  disabled={isInstalling}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium text-brand hover:text-brand-hover border border-brand/30 hover:border-brand/50 rounded-lg transition-colors disabled:opacity-50"
                >
                  <Download className="w-3 h-3" />
                  {isInstalling ? "Installing..." : "Install"}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
