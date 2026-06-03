import { formatRelativeTime } from "@utils/time";
import type { LatestAgentRun } from "../homeTypes";

type HomeLatestAgentsSectionProps = {
  isLoadingLatestAgents: boolean;
  latestAgentRuns: LatestAgentRun[];
  onSelectThread: (workspaceId: string, threadId: string) => void;
};

export function HomeLatestAgentsSection({
  isLoadingLatestAgents,
  latestAgentRuns,
  onSelectThread,
}: HomeLatestAgentsSectionProps) {
  return (
    <div className="flex flex-col gap-3">
      <div>
        <div className="text-ui-lg font-semibold text-text-strong">Latest agents</div>
      </div>
      {latestAgentRuns.length > 0 ? (
        <div className="grid grid-cols-3 max-[900px]:grid-cols-2 max-[640px]:grid-cols-1 gap-3">
          {latestAgentRuns.map((run) => (
            <button
              className="relative text-left cursor-pointer w-full p-3.5 pb-4 rounded-2xl bg-surface-card border border-border-strong transition-all duration-150 hover:-translate-y-px hover:shadow-[0_14px_28px_rgba(0,0,0,0.24)] focus-visible:outline-2 focus-visible:outline-border-accent focus-visible:outline-offset-2"
              style={{
                background:
                  "linear-gradient(135deg, rgba(100, 200, 255, 0.12), transparent 60%), var(--surface-card)",
              }}
              key={run.threadId}
              onClick={() => onSelectThread(run.workspaceId, run.threadId)}
              type="button"
            >
              <div className="flex items-baseline justify-between gap-2 mb-1">
                <div className="min-w-0 flex items-baseline gap-1.5">
                  <span className="text-ui-sm font-semibold text-text-strong truncate">{run.projectName}</span>
                  {run.groupName && <span className="text-ui-xs text-text-muted">{run.groupName}</span>}
                </div>
                <div className="text-ui-xs text-text-faint shrink-0">{formatRelativeTime(run.timestamp)}</div>
              </div>
              <div className="text-ui-sm text-text-muted line-clamp-2">{run.message.trim() || "Agent replied."}</div>
              {run.isProcessing && (
                <div className="text-ui-2xs text-status-success font-semibold mt-1">Running</div>
              )}
            </button>
          ))}
        </div>
      ) : isLoadingLatestAgents ? (
        <div
          className="grid grid-cols-3 max-[900px]:grid-cols-2 max-[640px]:grid-cols-1 gap-3 pointer-events-none"
          role="status"
          aria-label="Loading agents"
        >
          {[1, 2, 3].map((i) => (
            <div
              className="flex flex-col gap-2 p-3.5 pb-4 rounded-2xl bg-surface-card border border-border-strong"
              key={`skeleton-${i}`}
            >
              <div className="flex items-baseline justify-between gap-2 mb-1">
                <span className="h-2 w-24 rounded-full bg-surface-control" />
                <span className="h-2 w-12 rounded-full bg-surface-control" />
              </div>
              <span className="h-2 w-[70%] rounded-full bg-surface-control" />
              <span className="h-2 w-[40%] rounded-full bg-surface-control" />
            </div>
          ))}
        </div>
      ) : (
        <div className="flex flex-col gap-1.5 text-text-subtle">
          <div className="text-ui-lg text-text-stronger">No agent activity yet</div>
          <div className="text-ui-sm text-text-faint">
            Start a thread to see the latest responses here.
          </div>
        </div>
      )}
    </div>
  );
}
