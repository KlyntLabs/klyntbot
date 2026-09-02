import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import type {
  McpAddServerParams,
  McpConfigResponse,
  McpServerConfig,
  McpToggleServerParams,
  OAuthStartParams,
  RecommendedMcpServer,
} from "@shared/types";
import { Plug, Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AddServerDialog } from "../components/mcp/AddServerDialog";
import { CustomServerCard, RecommendedServerCard } from "../components/mcp/McpServerCard";
import { recommendedServers } from "../components/mcp/recommendedServers";

export function McpServersSettings() {
  const toast = useToastContext();
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
    if (oauthError) {
      console.error("[MCP OAuth] Error:", oauthError);
      toast.show(
        `OAuth failed: ${oauthError instanceof Error ? oauthError.message : String(oauthError)}`,
      );
    }
  }, [oauthError, toast]);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [prefillServer, setPrefillServer] = useState<RecommendedMcpServer | undefined>();
  const [editingServerName, setEditingServerName] = useState<string | null>(null);
  const [oauthLoadingServer, setOauthLoadingServer] = useState<string | null>(null);

  // Listen for OAuth completion/error events from the backend
  useEvent<{ serverName: string; provider: string }>("mcp:oauth_complete", () => {
    setOauthLoadingServer(null);
    refetch();
  });

  useEvent<{ serverName: string; error: string }>("mcp:oauth_error", (payload) => {
    setOauthLoadingServer(null);
    toast.show(`OAuth error for ${payload.serverName}: ${payload.error}`);
    refetch();
  });

  const handleAdd = useCallback(
    async (params: McpAddServerParams) => {
      if (editingServerName && editingServerName !== params.name) {
        await removeServer({ name: editingServerName });
      }
      await addServer(params);
      refetch();
      setDialogOpen(false);
      setPrefillServer(undefined);
      setEditingServerName(null);
    },
    [addServer, removeServer, refetch, editingServerName],
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
      setOauthLoadingServer(server.name);
      await startOAuth({ provider: server.oauthProvider, serverName: server.name });
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

  const handleEditCustom = useCallback((s: McpServerConfig) => {
    setPrefillServer({
      name: s.name,
      author: "",
      description: "",
      icon: "",
      transport: s.transport,
      command: s.command,
      args: s.args,
      envKeys: s.env ? Object.keys(s.env) : [],
      url: s.url,
    });
    setEditingServerName(s.name);
    setDialogOpen(true);
  }, []);

  const handleOpenAdd = useCallback(() => {
    setPrefillServer(undefined);
    setEditingServerName(null);
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
        <h2 className="text-lg font-medium text-fg">MCP servers</h2>
        <p className="text-ui text-fg-secondary mt-1">
          Connect external tools and data sources via the Model Context Protocol
        </p>
      </div>

      {/* Custom servers */}
      <div className="mb-8">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-ui font-medium text-fg-secondary">Custom servers</h3>
          <button
            type="button"
            onClick={handleOpenAdd}
            className="flex items-center gap-1.5 text-ui-sm text-brand hover:text-brand-hover transition-colors"
          >
            <Plus className="size-3.5" />
            Add server
          </button>
        </div>

        {customServers.length === 0 ? (
          <div className="island rounded-lg p-8 flex flex-col items-center text-center">
            <Plug className="size-8 text-fg-dim mb-3" strokeWidth={1.5} />
            <p className="text-ui text-fg-secondary">No custom MCP servers connected</p>
            <p className="text-ui-xs text-fg-dim mt-1">
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
                onEdit={handleEditCustom}
              />
            ))}
          </div>
        )}
      </div>

      {/* Recommended servers */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-ui font-medium text-fg-secondary">Recommended servers</h3>
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
          setEditingServerName(null);
        }}
        onAdd={handleAdd}
        prefill={prefillServer}
      />
    </div>
  );
}
