import { SettingsCard } from "@shared/composites/SettingsCard/SettingsCard";
import { useCopyToClipboard } from "@shared/hooks/useCopyToClipboard";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { Check, Copy, RefreshCw, Terminal, X } from "lucide-react";
import { useState } from "react";

interface ShellHookStatus {
  installed: boolean;
  shell: string;
  rcFile: string;
}

interface CaptureStatus {
  shellHookActive: boolean;
  eventCountLast24h: Record<string, number>;
}

export function IntegrationsSettings() {
  const { data: hookStatus, refetch: refetchHook } = useQuery<ShellHookStatus | null>(
    "capture_shell_hook_status",
    undefined,
    null,
  );
  const { data: captureStatus } = useQuery<CaptureStatus | null>("capture_status", undefined, null);
  const { data: token, refetch: refetchToken } = useQuery<string | null>(
    "capture_get_ingestion_token",
    undefined,
    null,
  );

  const [installing, setInstalling] = useState(false);
  const [tokenVisible, setTokenVisible] = useState(false);
  const { copied, copy } = useCopyToClipboard();

  const handleInstall = async () => {
    setInstalling(true);
    try {
      await ipc("capture_install_shell_hook");
      refetchHook();
    } finally {
      setInstalling(false);
    }
  };

  const handleUninstall = async () => {
    setInstalling(true);
    try {
      await ipc("capture_uninstall_shell_hook");
      refetchHook();
    } finally {
      setInstalling(false);
    }
  };

  const handleRegenerateToken = async () => {
    await ipc("capture_regenerate_ingestion_token");
    refetchToken();
  };

  const handleCopyToken = async () => {
    if (token) await copy(token);
  };

  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-lg font-medium text-foreground">Integrations</h2>
      <p className="text-[13px] text-muted-foreground -mt-4">
        Configure external capture sources to enrich your activity timeline.
      </p>

      {/* Shell Integration */}
      <SettingsCard title="Shell Integration">
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Terminal className="size-4 text-muted-foreground" strokeWidth={1.5} />
              <span className="text-[13px] text-muted-foreground">
                {hookStatus?.installed ? "Installed" : "Not installed"}
              </span>
              {hookStatus?.installed && <span className="size-2 rounded-full bg-success" />}
            </div>
            {hookStatus?.installed ? (
              <button
                type="button"
                onClick={handleUninstall}
                disabled={installing}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
              >
                <X className="size-3" />
                Uninstall
              </button>
            ) : (
              <button
                type="button"
                onClick={handleInstall}
                disabled={installing}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-brand/20 text-brand hover:bg-brand/30 transition-colors"
              >
                Install
              </button>
            )}
          </div>
          {hookStatus && (
            <div className="text-[11px] text-muted-foreground">
              Shell: {hookStatus.shell} · RC file: {hookStatus.rcFile}
            </div>
          )}
        </div>
      </SettingsCard>

      {/* Ingestion API */}
      <SettingsCard title="Ingestion API">
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-[13px] text-muted-foreground">Endpoint</span>
            <code className="text-xs text-muted-foreground bg-accent px-2 py-0.5 rounded">
              http://127.0.0.1:3456/api/v1/ingest
            </code>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-[13px] text-muted-foreground">Auth Token</span>
            <div className="flex items-center gap-2">
              <code className="text-xs text-muted-foreground bg-accent px-2 py-0.5 rounded max-w-[200px] truncate">
                {tokenVisible ? (token ?? "—") : "••••••••••••"}
              </code>
              <button
                type="button"
                onClick={() => setTokenVisible(!tokenVisible)}
                className="text-[11px] text-muted-foreground hover:text-foreground"
              >
                {tokenVisible ? "Hide" : "Show"}
              </button>
              <button
                type="button"
                onClick={handleCopyToken}
                className="text-muted-foreground hover:text-foreground"
                title="Copy token"
              >
                {copied ? (
                  <Check className="size-3.5 text-success" />
                ) : (
                  <Copy className="size-3.5" />
                )}
              </button>
              <button
                type="button"
                onClick={handleRegenerateToken}
                className="text-muted-foreground hover:text-foreground"
                title="Regenerate token"
              >
                <RefreshCw className="size-3.5" />
              </button>
            </div>
          </div>
        </div>
      </SettingsCard>

      {/* Event Counts */}
      {captureStatus?.eventCountLast24h &&
        Object.keys(captureStatus.eventCountLast24h).length > 0 && (
          <SettingsCard title="Event Sources (Last 24h)">
            <div className="flex flex-col gap-1.5">
              {Object.entries(captureStatus.eventCountLast24h).map(([source, count]) => (
                <div key={source} className="flex items-center justify-between text-xs">
                  <span className="text-muted-foreground capitalize">
                    {source.replace(/_/g, " ")}
                  </span>
                  <span className="text-muted-foreground tabular-nums">{count}</span>
                </div>
              ))}
            </div>
          </SettingsCard>
        )}

      {/* Browser Extension placeholder */}
      <SettingsCard title="Browser Extension">
        <p className="text-xs text-muted-foreground">
          Chrome extension coming soon. In the meantime, external plugins can send events to the
          ingestion API endpoint above.
        </p>
      </SettingsCard>
    </div>
  );
}
