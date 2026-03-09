import { CheckCircle, DollarSign, ExternalLink, FileText, X } from "lucide-react";
import { useNavigate } from "react-router";
import { formatHumanDuration } from "../../lib/dates";
import type { TimelineEntry, TimelineSummary } from "../../lib/types";

interface SummaryPanelProps {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  onClose: () => void;
}

export function SummaryPanel({ summary, selectedEntry, onClose }: SummaryPanelProps) {
  const navigate = useNavigate();

  if (selectedEntry) {
    return <EntryDetail entry={selectedEntry} onClose={onClose} onNavigate={navigate} />;
  }
  if (!summary) return null;
  return <DefaultSummary summary={summary} />;
}

const SOURCE_ORDER: Record<string, number> = {
  focus: 0,
  task: 1,
  productivity: 2,
  note: 3,
  finance: 4,
  system: 5,
};

function DefaultSummary({ summary }: { summary: TimelineSummary }) {
  const sortedBreakdown = [...summary.sourceBreakdown].sort(
    (a, b) => (SOURCE_ORDER[a.source] ?? 9) - (SOURCE_ORDER[b.source] ?? 9),
  );

  return (
    <div className="w-72 glass-card p-4 flex flex-col gap-4 overflow-y-auto">
      <h3 className="text-xs font-semibold text-muted uppercase tracking-wider">Summary</h3>

      {/* Focus — primary stat */}
      <div className="p-2 rounded-lg bg-timeline-focus/10 border border-timeline-focus/20">
        <div className="text-lg font-semibold text-primary">
          {formatHumanDuration(summary.focusSecs)}
        </div>
        <div className="text-[10px] text-muted">Focus time</div>
        {summary.totalTrackedSecs > 0 && (
          <div className="text-[10px] text-brand mt-0.5">
            {Math.round((summary.focusSecs / summary.totalTrackedSecs) * 100)}% focus ratio
          </div>
        )}
      </div>

      {/* Total tracked */}
      <div>
        <div className="text-sm font-semibold text-primary">
          {formatHumanDuration(summary.totalTrackedSecs)}
        </div>
        <div className="text-[10px] text-muted">total tracked</div>
      </div>

      {/* Quick stats */}
      <div className="grid grid-cols-2 gap-2">
        <Stat
          icon={<CheckCircle className="w-3.5 h-3.5" />}
          label="Completed"
          value={String(summary.tasksCompleted)}
        />
        <Stat
          icon={<FileText className="w-3.5 h-3.5" />}
          label="Notes"
          value={String(summary.notesTouched)}
        />
        <Stat
          icon={<DollarSign className="w-3.5 h-3.5" />}
          label="Transactions"
          value={String(summary.transactionsCount)}
        />
      </div>

      {/* Top apps */}
      {summary.topApps.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted mb-2">Top Apps</h4>
          <div className="flex flex-col gap-1.5">
            {summary.topApps.map((app) => (
              <div key={app.appName} className="flex items-center justify-between text-xs">
                <span className="text-secondary truncate">{app.appName}</span>
                <span className="text-muted">{formatHumanDuration(app.durationSecs)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Source breakdown */}
      {sortedBreakdown.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted mb-2">Breakdown</h4>
          <div className="flex flex-col gap-1.5">
            {sortedBreakdown.map((s) => (
              <div key={s.source} className="flex items-center justify-between text-xs">
                <span className="text-secondary capitalize">{s.source}</span>
                <span className="text-muted">{s.count} items</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function EntryDetail({
  entry,
  onClose,
  onNavigate,
}: {
  entry: TimelineEntry;
  onClose: () => void;
  onNavigate: (path: string) => void;
}) {
  return (
    <div className="w-72 glass-card p-4 flex flex-col gap-3 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-muted uppercase tracking-wider">Details</h3>
        <button type="button" onClick={onClose} className="text-muted hover:text-secondary">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex items-center gap-2">
        <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: entry.color }} />
        <span className="text-sm font-medium text-primary">{entry.title}</span>
      </div>

      {entry.description && <p className="text-xs text-muted">{entry.description}</p>}

      <div className="text-xs text-muted space-y-1">
        <div>Started: {new Date(entry.startedAt).toLocaleTimeString()}</div>
        {entry.endedAt && <div>Ended: {new Date(entry.endedAt).toLocaleTimeString()}</div>}
        {entry.durationSecs != null && entry.durationSecs > 0 && (
          <div>Duration: {formatHumanDuration(entry.durationSecs)}</div>
        )}
        <div className="capitalize">Source: {entry.source}</div>
      </div>

      {entry.entityRoute != null && (
        <button
          type="button"
          onClick={() => onNavigate(entry.entityRoute as string)}
          className="flex items-center gap-1.5 text-xs text-brand hover:underline mt-1"
        >
          <ExternalLink className="w-3.5 h-3.5" />
          Open {entry.source}
        </button>
      )}
    </div>
  );
}

function Stat({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="glass-card p-2 flex flex-col gap-0.5">
      <div className="flex items-center gap-1 text-muted">
        {icon}
        <span className="text-[10px]">{label}</span>
      </div>
      <div className="text-sm font-semibold text-primary">{value}</div>
    </div>
  );
}
