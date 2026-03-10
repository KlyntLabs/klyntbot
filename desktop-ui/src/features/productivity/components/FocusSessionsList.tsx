import { useQuery } from "@shared/hooks/useQuery";
import { formatTime } from "@shared/lib/dates";
import type { FocusSession } from "@shared/types";

interface FocusSessionsListProps {
  date: string;
}

function qualityBadge(score: number | null): { text: string; color: string } {
  if (score == null) return { text: "—", color: "text-dim" };
  const pct = Math.round(score * 100);
  if (pct >= 80) return { text: `${pct}%`, color: "text-success" };
  if (pct >= 50) return { text: `${pct}%`, color: "text-brand" };
  return { text: `${pct}%`, color: "text-destructive" };
}

export function FocusSessionsList({ date }: FocusSessionsListProps) {
  const { data: sessions } = useQuery<FocusSession[]>("productivity_sessions", { date }, []);

  if (sessions.length === 0) {
    return (
      <div className="glass-card p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Focus Sessions</h2>
        <p className="text-[12px] font-light text-dim">No sessions today</p>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Focus Sessions</h2>
      <div className="flex flex-col gap-0.5">
        {sessions.map((s) => {
          const quality = qualityBadge(s.qualityScore);
          return (
            <div
              key={s.id}
              className="flex items-center gap-3 py-2 border-b border-white/[0.04] last:border-b-0"
            >
              <span className="text-[11px] font-light text-muted tabular-nums w-14">
                {formatTime(s.startedAt)}
              </span>
              <span className="text-[11px] font-light text-primary flex-1 truncate">
                {s.sessionType === "pomodoro" ? "Pomodoro" : "Focus"}
                {s.notes ? ` — ${s.notes}` : ""}
              </span>
              <span className="text-[11px] font-light text-muted tabular-nums">
                {s.actualMins != null ? `${s.actualMins} min` : "In progress"}
              </span>
              <span className={`text-[11px] font-light tabular-nums ${quality.color}`}>
                {quality.text}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
