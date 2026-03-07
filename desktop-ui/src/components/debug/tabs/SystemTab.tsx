import { AlertCircle, CheckCircle2, Circle } from "lucide-react";
import { useMemo } from "react";
import { useQuery } from "../../../hooks/useQuery";

interface ComponentStatus {
  name: string;
  status: string;
  handlerType: string;
  notes: string;
}

interface SystemStatus {
  domainBusSubscribers: number;
  domainBusPublished: number;
  backgroundServiceRunning: boolean;
  backgroundEventsProcessed: number;
  activeFacts: number;
  episodicCount: number;
  rulesCount: number;
  components: ComponentStatus[];
}

const statusConfig: Record<string, { icon: typeof CheckCircle2; color: string; bg: string }> = {
  wired: { icon: CheckCircle2, color: "text-green-400", bg: "bg-green-500/20" },
  built: { icon: AlertCircle, color: "text-yellow-400", bg: "bg-yellow-500/20" },
  stub: { icon: Circle, color: "text-muted", bg: "bg-white/[0.06]" },
};

export function SystemTab() {
  const { data: system } = useQuery<SystemStatus>("cognitive_system_status", undefined, {
    domainBusSubscribers: 0,
    domainBusPublished: 0,
    backgroundServiceRunning: false,
    backgroundEventsProcessed: 0,
    activeFacts: 0,
    episodicCount: 0,
    rulesCount: 0,
    components: [],
  });

  const {
    wired: wiredCount,
    built: builtCount,
    stub: stubCount,
  } = useMemo(
    () =>
      system.components.reduce(
        (acc, c) => {
          if (c.status in acc) acc[c.status as keyof typeof acc]++;
          return acc;
        },
        { wired: 0, built: 0, stub: 0 },
      ),
    [system.components],
  );

  return (
    <div className="space-y-6">
      {/* Service Health Cards */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">Service Health</h2>
        <div className="grid grid-cols-4 gap-3">
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Domain Event Bus</h3>
            <p className="text-[13px] text-secondary">{system.domainBusSubscribers} subscribers</p>
            <p className="text-[11px] text-muted">{system.domainBusPublished} published</p>
          </div>
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Background Consolidation</h3>
            <p className="text-[13px] text-secondary">
              {system.backgroundServiceRunning ? (
                <span className="text-green-400">Running</span>
              ) : (
                <span className="text-red-400">Stopped</span>
              )}
            </p>
            <p className="text-[11px] text-muted">{system.backgroundEventsProcessed} processed</p>
          </div>
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Memory System</h3>
            <p className="text-[13px] text-secondary">{system.activeFacts} active facts</p>
            <p className="text-[11px] text-muted">
              {system.episodicCount} episodic / {system.rulesCount} rules
            </p>
          </div>
          <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
            <h3 className="text-[11px] text-muted mb-2">Implementation</h3>
            <p className="text-[13px] text-secondary">
              <span className="text-green-400">{wiredCount}</span> /{" "}
              <span className="text-yellow-400">{builtCount}</span> /{" "}
              <span className="text-muted">{stubCount}</span>
            </p>
            <p className="text-[11px] text-muted">wired / built / stub</p>
          </div>
        </div>
      </div>

      {/* Implementation Completeness Matrix */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">Implementation Completeness</h2>
        <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] overflow-hidden">
          <table className="w-full text-[12px]">
            <thead>
              <tr className="border-b border-white/[0.06]">
                <th className="text-left p-2 text-muted font-normal">Component</th>
                <th className="text-left p-2 text-muted font-normal">Status</th>
                <th className="text-left p-2 text-muted font-normal">Handler</th>
                <th className="text-left p-2 text-muted font-normal">Notes</th>
              </tr>
            </thead>
            <tbody>
              {system.components.map((c) => {
                const cfg = statusConfig[c.status] ?? statusConfig.stub;
                const Icon = cfg.icon;
                return (
                  <tr key={c.name} className="border-b border-white/[0.04]">
                    <td className="p-2 text-secondary">{c.name}</td>
                    <td className="p-2">
                      <span
                        className={`inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded ${cfg.bg} ${cfg.color}`}
                      >
                        <Icon className="w-3 h-3" />
                        {c.status}
                      </span>
                    </td>
                    <td className="p-2">
                      <span className="text-[10px] text-muted bg-white/[0.06] px-1.5 py-0.5 rounded">
                        {c.handlerType}
                      </span>
                    </td>
                    <td className="p-2 text-muted">{c.notes}</td>
                  </tr>
                );
              })}
              {system.components.length === 0 && (
                <tr>
                  <td colSpan={4} className="p-4 text-center text-muted">
                    No component data
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Legend */}
      <div className="flex gap-4 text-[11px]">
        <span className="flex items-center gap-1 text-green-400">
          <CheckCircle2 className="w-3 h-3" /> Wired — fully integrated and running
        </span>
        <span className="flex items-center gap-1 text-yellow-400">
          <AlertCircle className="w-3 h-3" /> Built — code exists but not connected
        </span>
        <span className="flex items-center gap-1 text-muted">
          <Circle className="w-3 h-3" /> Stub — trait defined, implementation pending
        </span>
      </div>
    </div>
  );
}
