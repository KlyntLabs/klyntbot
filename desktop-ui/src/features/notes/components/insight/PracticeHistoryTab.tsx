import { useQuery } from "@shared/hooks/useQuery";
import { useMemo } from "react";

/** Matches PracticeSessionResponse from the backend (camelCase). */
interface PracticeSession {
  id: string;
  noteId: string;
  status: string;
  segments: string; // JSON array
  currentIndex: number;
  results: string; // JSON array
  averageScore: number | null;
  startedAt: string;
  completedAt: string | null;
}

interface PracticeHistoryTabProps {
  noteId: string | null;
}

function formatSessionDate(iso: string): string {
  const d = new Date(iso);
  const month = d.toLocaleString(undefined, { month: "short" });
  const day = d.getDate();
  const year = d.getFullYear();
  const time = d.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
  return `${month} ${day}, ${year} \u00b7 ${time}`;
}

function statusBadge(status: string): { label: string; className: string } {
  switch (status) {
    case "completed":
      return {
        label: "Completed",
        className: "bg-status-success/15 text-status-success",
      };
    case "in_progress":
      return {
        label: "In Progress",
        className: "bg-status-warning/15 text-status-warning",
      };
    default:
      return {
        label: "Abandoned",
        className: "bg-fg-secondary/15 text-fg-secondary",
      };
  }
}

export function PracticeHistoryTab({ noteId }: PracticeHistoryTabProps) {
  const { data: sessions, loading } = useQuery<PracticeSession[]>(
    "practice_list_sessions",
    noteId ? { params: { noteId } } : null,
    [],
  );

  const segmentCounts = useMemo(
    () =>
      new Map(
        (sessions ?? []).map((s) => {
          try {
            return [s.id, (JSON.parse(s.segments) as unknown[]).length] as const;
          } catch {
            return [s.id, 0] as const;
          }
        }),
      ),
    [sessions],
  );

  if (!noteId) {
    return (
      <p className="text-fg-secondary text-sm text-center py-8">
        Select a note to see practice history
      </p>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <span className="text-fg-secondary text-ui-sm animate-pulse">Loading sessions...</span>
      </div>
    );
  }

  if (!sessions || sessions.length === 0) {
    return (
      <p className="text-fg-secondary text-sm text-center py-8">
        No practice sessions yet. Select text and tap &lsquo;Practice this note&rsquo; to start.
      </p>
    );
  }

  return (
    <div className="space-y-2">
      {sessions.map((session) => {
        const badge = statusBadge(session.status);
        const totalUnits = segmentCounts.get(session.id) ?? 0;
        const completedUnits = session.currentIndex;
        const stats =
          totalUnits > 0 ? `${completedUnits}/${totalUnits} units` : `${completedUnits} units`;
        const avgLabel =
          session.averageScore != null ? ` \u00b7 ${session.averageScore.toFixed(1)}` : "";

        return (
          <div
            key={session.id}
            className="rounded-lg border border-separator bg-glass-subtle/50 p-3 space-y-1.5"
          >
            <div className="flex items-center justify-between">
              <span className="text-ui-xs text-fg-secondary">
                {formatSessionDate(session.startedAt)}
              </span>
              <span
                className={`text-ui-xs font-medium rounded-full px-2 py-0.5 ${badge.className}`}
              >
                {badge.label}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-ui-sm text-fg">
                {stats}
                {avgLabel}
              </span>
              {session.status === "in_progress" && (
                <button
                  type="button"
                  className="text-ui-xs font-medium text-brand hover:text-brand/80 transition-colors"
                >
                  Resume
                </button>
              )}
              {session.status === "completed" && (
                <button
                  type="button"
                  className="text-ui-xs font-medium text-fg-secondary hover:text-fg transition-colors"
                >
                  Review
                </button>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
