import { useQuery } from "../../../hooks/useQuery";
import type { AgentStatus } from "../../../lib/types";

export function GeneralSettings() {
  const { data: status } = useQuery<AgentStatus>("agent_status", undefined, {
    status: "unknown",
    activeTaskCount: 0,
    focusTask: null,
  });

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">General</h2>
        <p className="text-[13px] text-muted mt-1">Overview and system information</p>
      </div>

      <div className="space-y-4">
        <div className="bg-surface-low rounded-lg border border-border p-4">
          <h3 className="text-[13px] font-medium text-secondary mb-3">System</h3>
          <div className="space-y-2">
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Version</span>
              <span className="text-secondary font-mono">0.1.0</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Data directory</span>
              <span className="text-secondary font-mono">~/.klyntbot</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Agent status</span>
              <span className="text-secondary">{status.status}</span>
            </div>
            <div className="flex justify-between text-[13px]">
              <span className="text-muted">Active tasks</span>
              <span className="text-secondary">{status.activeTaskCount}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
