import type { McpServerConfig, RecommendedMcpServer } from "@shared/types";
import { Check, Loader2, Power, Settings2, Trash2 } from "lucide-react";
import { McpServerIcon } from "./McpServerIcon";

interface CustomServerCardProps {
  server: McpServerConfig;
  onToggle: (name: string, enabled: boolean) => void;
  onRemove: (name: string) => void;
  onEdit: (server: McpServerConfig) => void;
}

export function CustomServerCard({ server, onToggle, onRemove, onEdit }: CustomServerCardProps) {
  return (
    <div className="flex items-center justify-between py-3 px-4 bg-card rounded-lg border border-border">
      <div className="flex items-center gap-3 min-w-0">
        <div className="w-8 h-8 rounded-md bg-muted flex items-center justify-center text-muted-foreground flex-shrink-0">
          <McpServerIcon name={server.name} className="w-4 h-4" />
        </div>
        <div className="min-w-0">
          <div className="text-[13px] font-medium text-foreground truncate">{server.name}</div>
          <div className="text-[11px] text-dim">
            {server.transport === "stdio" ? server.command : server.url}
          </div>
        </div>
      </div>

      <div className="flex items-center gap-1.5 flex-shrink-0">
        <span
          className={`text-[11px] px-1.5 py-0.5 rounded ${
            server.transport === "stdio" ? "bg-purple/10 text-purple" : "bg-info/10 text-info"
          }`}
        >
          {server.transport}
        </span>
        <button
          type="button"
          onClick={() => onToggle(server.name, !server.enabled)}
          className={`w-7 h-7 rounded-md flex items-center justify-center transition-colors ${
            server.enabled ? "text-success hover:bg-accent" : "text-dim hover:bg-accent"
          }`}
          aria-label={server.enabled ? "Disable" : "Enable"}
        >
          <Power className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
        <button
          type="button"
          onClick={() => onEdit(server)}
          className="w-7 h-7 rounded-md flex items-center justify-center text-muted-foreground hover:text-muted-foreground hover:bg-accent transition-colors"
          aria-label="Edit"
        >
          <Settings2 className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
        <button
          type="button"
          onClick={() => onRemove(server.name)}
          className="w-7 h-7 rounded-md flex items-center justify-center text-muted-foreground hover:text-destructive hover:bg-accent transition-colors"
          aria-label="Remove"
        >
          <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      </div>
    </div>
  );
}

interface RecommendedServerCardProps {
  server: RecommendedMcpServer;
  installed: boolean;
  enabled?: boolean;
  oauthConnected?: boolean;
  oauthLoading?: boolean;
  onInstall: (server: RecommendedMcpServer) => void;
  onToggle?: (name: string, enabled: boolean) => void;
  onOAuthConnect?: (server: RecommendedMcpServer) => void;
  onOAuthDisconnect?: (serverName: string) => void;
  onEdit?: (server: RecommendedMcpServer) => void;
}

export function RecommendedServerCard({
  server,
  installed,
  enabled,
  oauthConnected,
  oauthLoading,
  onInstall,
  onToggle,
  onOAuthConnect,
  onOAuthDisconnect,
  onEdit,
}: RecommendedServerCardProps) {
  const hasOAuth = !!server.oauthProvider;
  const needsAuth = installed && hasOAuth && !oauthConnected;
  const isConnected = installed && hasOAuth && oauthConnected;

  return (
    <div className="flex items-center justify-between py-3 px-4 bg-card rounded-lg border border-border">
      <div className="flex items-center gap-3 min-w-0">
        <div className="w-8 h-8 rounded-md bg-muted flex items-center justify-center text-muted-foreground flex-shrink-0">
          <McpServerIcon name={server.name} className="w-4 h-4" />
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-[13px] font-medium text-foreground">{server.name}</span>
            <span className="text-[11px] text-dim">by {server.author}</span>
          </div>
          <div className="text-[11px] text-muted-foreground truncate">{server.description}</div>
        </div>
      </div>

      <div className="flex items-center gap-1.5 flex-shrink-0 ml-3">
        {!installed ? (
          /* State 1: Not installed → Install button */
          <button
            type="button"
            onClick={() => onInstall(server)}
            className="text-[12px] px-3 py-1 rounded-md border border-brand/30 text-brand bg-brand/5 hover:bg-brand/10 transition-colors"
          >
            Install
          </button>
        ) : (
          /* Installed states */
          <>
            {needsAuth && (
              /* State 2: Installed, needs auth → Authenticate button */
              <button
                type="button"
                onClick={() => onOAuthConnect?.(server)}
                disabled={oauthLoading}
                className="text-[12px] px-3 py-1 rounded-md border border-brand/30 text-brand bg-brand/5 hover:bg-brand/10 transition-colors disabled:opacity-50 flex items-center gap-1.5"
              >
                {oauthLoading ? (
                  <>
                    <Loader2 className="w-3 h-3 animate-spin" />
                    Waiting\u2026
                  </>
                ) : (
                  "Authenticate"
                )}
              </button>
            )}

            {isConnected && (
              /* State 3: Installed + authenticated → Connected badge */
              <button
                type="button"
                onClick={() => onOAuthDisconnect?.(server.name)}
                className="text-[12px] px-3 py-1 rounded-md border border-success/30 text-success bg-success/5 hover:bg-destructive/10 hover:text-destructive hover:border-destructive/30 transition-colors flex items-center gap-1"
                title="Click to disconnect"
              >
                <Check className="w-3 h-3" />
                Connected
              </button>
            )}

            {/* No OAuth provider — show simple enabled/disabled for non-OAuth servers */}
            {!hasOAuth && (
              <button
                type="button"
                onClick={() => onToggle?.(server.name, !enabled)}
                className={`text-[12px] px-3 py-1 rounded-md border transition-colors ${
                  enabled
                    ? "border-success/30 text-success bg-success/5 hover:bg-success/10"
                    : "border-border text-dim bg-accent hover:bg-muted"
                }`}
              >
                {enabled ? "Enabled" : "Disabled"}
              </button>
            )}

            {/* Gear icon — edit config */}
            <button
              type="button"
              onClick={() => onEdit?.(server)}
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted-foreground hover:text-muted-foreground hover:bg-accent transition-colors"
              aria-label="Configure"
            >
              <Settings2 className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>

            {/* Toggle on/off (for OAuth servers too) */}
            {hasOAuth && (
              <button
                type="button"
                onClick={() => onToggle?.(server.name, !enabled)}
                className={`w-7 h-7 rounded-md flex items-center justify-center transition-colors ${
                  enabled ? "text-success hover:bg-accent" : "text-dim hover:bg-accent"
                }`}
                aria-label={enabled ? "Disable" : "Enable"}
              >
                <Power className="w-3.5 h-3.5" strokeWidth={1.5} />
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
