import { SettingsCard } from "@shared/composites/SettingsCard/SettingsCard";
import { useMutation, useQuery } from "@shared/hooks";
import type { CodingMemoryStatusResponse, DiagnoseResult } from "@shared/types";
import { Activity, Terminal } from "lucide-react";
import { useState } from "react";

export function CodingCliSettings() {
  const status = useQuery<CodingMemoryStatusResponse | null>(
    "coding_memory_status",
    undefined,
    null,
  );
  const enable = useMutation<void, { cli: string }>("coding_memory_enable_cli", "params");
  const disable = useMutation<void, { cli: string }>("coding_memory_disable_cli", "params");
  const diagnose = useMutation<DiagnoseResult, { cli: string }>(
    "coding_memory_diagnose_cli",
    "params",
  );
  const [lastDiagnose, setLastDiagnose] = useState<DiagnoseResult | null>(null);

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h2 className="text-lg font-medium text-foreground">Coding CLI Integration</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Klyntbot can observe your coding CLI sessions via hooks. Memory accumulates silently and
          surfaces on your next session.
        </p>
      </div>

      <SettingsCard title="Claude Code">
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Terminal className="size-4 text-muted-foreground" strokeWidth={1.5} />
              <span className="text-[13px] text-muted-foreground">
                Writes hook config to ~/.claude/settings.json
              </span>
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={async () => {
                  await enable.mutate({ cli: "claude-code" });
                  status.refetch();
                }}
                disabled={enable.loading}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-brand/20 text-brand hover:bg-brand/30 transition-colors disabled:opacity-50"
              >
                Enable
              </button>
              <button
                type="button"
                onClick={async () => {
                  await disable.mutate({ cli: "claude-code" });
                  status.refetch();
                }}
                disabled={disable.loading}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-50"
              >
                Disable
              </button>
              <button
                type="button"
                onClick={async () => {
                  const r = await diagnose.mutate({ cli: "claude-code" });
                  if (r) setLastDiagnose(r);
                }}
                disabled={diagnose.loading}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-50"
              >
                Diagnose
              </button>
            </div>
          </div>
          {lastDiagnose && (
            <div
              className={`rounded-lg p-2 text-xs ${
                lastDiagnose.ok
                  ? "bg-success/10 text-success"
                  : "bg-destructive/10 text-destructive"
              }`}
            >
              {lastDiagnose.message}
            </div>
          )}
        </div>
      </SettingsCard>

      <SettingsCard title="Daemon Status">
        {status.data ? (
          <div className="grid grid-cols-2 gap-y-2 text-[13px]">
            <span className="text-muted-foreground">Socket</span>
            <span className="text-foreground font-mono text-xs">{status.data.socketPath}</span>
            <span className="text-muted-foreground">Daemon</span>
            <span className="text-foreground">
              {status.data.daemonAlive ? (
                <span className="inline-flex items-center gap-1.5">
                  <span className="size-2 rounded-full bg-success" />
                  alive
                </span>
              ) : (
                <span className="inline-flex items-center gap-1.5">
                  <span className="size-2 rounded-full bg-destructive" />
                  stopped
                </span>
              )}
            </span>
            <span className="text-muted-foreground">Buffered bytes</span>
            <span className="text-foreground tabular-nums">{status.data.bufferedEventCount}</span>
            <span className="text-muted-foreground">Unprocessed events</span>
            <span className="text-foreground tabular-nums">
              {status.data.unprocessedEventCount}
            </span>
          </div>
        ) : (
          <div className="text-[13px] text-muted-foreground">Loading…</div>
        )}
      </SettingsCard>

      <div className="flex items-start gap-2 text-[11px] text-muted-foreground">
        <Activity className="size-3.5 mt-0.5 flex-shrink-0" strokeWidth={1.5} />
        <p>
          Klyntbot desktop must be running in the background for hooks to forward events
          immediately. When the desktop is off, events buffer to disk and drain on next startup.
        </p>
      </div>
    </div>
  );
}
