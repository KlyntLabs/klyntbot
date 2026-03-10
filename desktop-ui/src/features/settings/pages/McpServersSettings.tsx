import { Plug, Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type {
  McpAddServerParams,
  McpConfigResponse,
  McpToggleServerParams,
  OAuthStartParams,
  RecommendedMcpServer,
} from "@shared/types";
import { AddServerDialog } from "../components/mcp/AddServerDialog";
import { CustomServerCard, RecommendedServerCard } from "../components/mcp/McpServerCard";
import { recommendedServers } from "../components/mcp/recommendedServers";

export function McpServersSettings() {
  const { data: config, refetch } = useQuery<McpConfigResponse>("mcp_get_config", undefined, {
    enabled: true,
    servers: [],
  });

  const { mutate: addServer } = useMutation<McpConfigResponse, McpAddServerParams>(
    "mcp_add_server",
    "params",
  );
  const { mutate: toggleServer } = useMutation<McpConfigResponse, McpToggleServerParams>(
    "mcp_toggle_server",
    "params",
  );
  const { mutate: removeServer } = useMutation<McpConfigResponse, { name: string }>(
    "mcp_remove_server",
    "params",
  );
  const { mutate: startOAuth, error: oauthError } = useMutation<void, OAuthStartParams>(
    "mcp_oauth_start",
    "params",
  );
  const { mutate: disconnectOAuth } = useMutation<McpConfigResponse, { serverName: string }>(
    "mcp_oauth_disconnect",
  );

  useEffect(() => {
    if (oauthError) console.error("[MCP OAuth] Error:", oauthError);
  }, [oauthError]);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [prefillServer, setPrefillServer] = useState<RecommendedMcpServer | undefined>();
  const [oauthLoadingServer, setOauthLoadingServer] = useState<string | null>(null);

  // Listen for OAuth completion/error events from the backend
  useEvent<{ serverName: string; provider: string }>("mcp:oauth_complete", () => {
    setOauthLoadingServer(null);
    refetch();
  });

  useEvent<{ serverName: string; error: string }>("mcp:oauth_error", () => {
    setOauthLoadingServer(null);
    refetch();
  });

  const handleAdd = useCallback(
    async (params: McpAddServerParams) => {
      await addServer(params);
      refetch();
      setDialogOpen(false);
      setPrefillServer(undefined);
    },
    [addServer, refetch],
  );

  const handleToggle = useCallback(
    async (name: string, enabled: boolean) => {
      await toggleServer({ name, enabled });
      refetch();
    },
    [toggleServer, refetch],
  );

  const [confirmRemoveServer, setConfirmRemoveServer] = useState<string | null>(null);

  const handleRemove = useCallback(
    async (name: string) => {
      if (confirmRemoveServer !== name) {
        setConfirmRemoveServer(name);
        return;
      }
      setConfirmRemoveServer(null);
      await removeServer({ name });
      refetch();
    },
    [removeServer, refetch, confirmRemoveServer],
  );

  // One-click install: add server with defaults (no dialog for recommended servers)
  const handleInstall = useCallback(
    async (server: RecommendedMcpServer) => {
      if (!server.command) {
        // Fallback: open dialog for servers without a default command
        setPrefillServer(server);
        setDialogOpen(true);
        return;
      }
      await addServer({
        name: server.name,
        transport: server.transport,
        command: server.command,
        args: server.args,
        env: {},
      });
      refetch();
    },
    [addServer, refetch],
  );

  // Start OAuth flow for an installed server
  const handleOAuthConnect = useCallback(
    async (server: RecommendedMcpServer) => {
      if (!server.oauthProvider) return;
      console.log("[MCP OAuth] Starting flow for", server.name, server.oauthProvider);
      setOauthLoadingServer(server.name);
      const result = await startOAuth({ provider: server.oauthProvider, serverName: server.name });
      console.log("[MCP OAuth] Result:", result);
    },
    [startOAuth],
  );

  // Disconnect OAuth
  const handleOAuthDisconnect = useCallback(
    async (serverName: string) => {
      await disconnectOAuth({ serverName });
      refetch();
    },
    [disconnectOAuth, refetch],
  );

  // Open edit dialog for a recommended server
  const handleEditRecommended = useCallback((server: RecommendedMcpServer) => {
    setPrefillServer(server);
    setDialogOpen(true);
  }, []);

  const handleOpenAdd = useCallback(() => {
    setPrefillServer(undefined);
    setDialogOpen(true);
  }, []);

  const installedNames = useMemo(
    () => new Set(config.servers.map((s) => s.name)),
    [config.servers],
  );

  const customServers = useMemo(
    () => config.servers.filter((s) => !recommendedServers.some((r) => r.name === s.name)),
    [config.servers],
  );

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">MCP servers</h2>
        <p className="text-[13px] text-muted mt-1">
          Connect external tools and data sources via the Model Context Protocol
        </p>
      </div>

      {/* Custom servers */}
      <div className="mb-8">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-[13px] font-medium text-secondary">Custom servers</h3>
          <button
            type="button"
            onClick={handleOpenAdd}
            className="flex items-center gap-1.5 text-[12px] text-brand hover:text-brand-hover transition-colors"
          >
            <Plus className="w-3.5 h-3.5" />
            Add server
          </button>
        </div>

        {customServers.length === 0 ? (
          <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] p-8 flex flex-col items-center text-center">
            <Plug className="w-8 h-8 text-dim mb-3" strokeWidth={1.5} />
            <p className="text-[13px] text-muted">No custom MCP servers connected</p>
            <p className="text-[11px] text-dim mt-1">
              Add a server manually or install one from the recommended list below
            </p>
          </div>
        ) : (
          <div className="space-y-1.5">
            {customServers.map((server) => (
              <CustomServerCard
                key={server.name}
                server={server}
                onToggle={handleToggle}
                onRemove={handleRemove}
                onEdit={() => {
                  // Edit is a future enhancement — for now, remove + re-add
                }}
              />
            ))}
          </div>
        )}
      </div>

      {/* Recommended servers */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-[13px] font-medium text-secondary">Recommended servers</h3>
        </div>

        <div className="space-y-1.5">
          {recommendedServers.map((server) => {
            const installed = installedNames.has(server.name);
            const configServer = config.servers.find((s) => s.name === server.name);
            return (
              <RecommendedServerCard
                key={server.name}
                server={server}
                installed={installed}
                enabled={configServer?.enabled}
                oauthConnected={configServer?.oauthConnected}
                oauthLoading={oauthLoadingServer === server.name}
                onInstall={handleInstall}
                onToggle={handleToggle}
                onOAuthConnect={handleOAuthConnect}
                onOAuthDisconnect={handleOAuthDisconnect}
                onEdit={handleEditRecommended}
              />
            );
          })}
        </div>
      </div>

      <AddServerDialog
        key={prefillServer?.name ?? "__new__"}
        open={dialogOpen}
        onClose={() => {
          setDialogOpen(false);
          setPrefillServer(undefined);
        }}
        onAdd={handleAdd}
        prefill={prefillServer}
      />
    </div>
  );
}
