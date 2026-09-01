import { useCliHealth, useCodingMemoryStatus } from "./hooks";

const CLI_LABELS: Record<string, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
  "kimi-cli": "kimi-cli",
  opencode: "opencode",
};

export function CliHealthPanel() {
  const rows = useCliHealth();
  const status = useCodingMemoryStatus();

  if (!rows.data || !status.data) {
    return <div className="p-6 text-sm text-muted-foreground">Loading…</div>;
  }

  return (
    <section className="p-6 space-y-4">
      <header className="flex items-baseline justify-between">
        <h2 className="text-lg font-medium text-foreground">CLI Health</h2>
        <span
          className={`rounded-lg px-2 py-0.5 text-xs ${
            status.data.daemonAlive
              ? "bg-success/10 text-success"
              : "bg-destructive/10 text-destructive"
          }`}
        >
          Daemon {status.data.daemonAlive ? "alive" : "stopped"}
        </span>
      </header>
      <div className="rounded-xl border border-border overflow-hidden">
        <table className="w-full text-[13px]">
          <thead className="text-[11px] uppercase tracking-wider text-muted-foreground bg-accent/30">
            <tr>
              <th className="py-2 px-3 text-left font-medium">CLI</th>
              <th className="py-2 px-3 text-left font-medium">Enabled</th>
              <th className="py-2 px-3 text-left font-medium">Last event</th>
              <th className="py-2 px-3 text-right font-medium">24h events</th>
            </tr>
          </thead>
          <tbody>
            {rows.data.map((r) => (
              <tr
                key={r.cli}
                className="border-t border-border hover:bg-accent/20 transition-colors"
              >
                <td className="py-2 px-3 text-foreground">{CLI_LABELS[r.cli] ?? r.cli}</td>
                <td className="py-2 px-3 text-muted-foreground">{r.enabled ? "yes" : "no"}</td>
                <td className="py-2 px-3 text-muted-foreground">{r.lastEventAt ?? "—"}</td>
                <td className="py-2 px-3 text-right text-muted-foreground tabular-nums">
                  {r.eventCount24h}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
