import { Play, Square, Timer } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { formatElapsed } from "@shared/lib/dates";
import type { FocusSession } from "@shared/types";

export function PomodoroTimer() {
  const { data: session, refetch } = useQuery<FocusSession | null>(
    "productivity_focus_status",
    undefined,
    null,
  );
  const startFocus = useMutation<FocusSession, { target_mins?: number }>(
    "productivity_focus_start",
  );
  const startPomodoro = useMutation<FocusSession, { work_mins?: number; break_mins?: number }>(
    "productivity_pomodoro_start",
  );
  const endFocus = useMutation<FocusSession | null, { notes?: string }>("productivity_focus_end");

  const [elapsed, setElapsed] = useState(0);

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "focus_session") refetch();
  });

  const startedAt = session?.startedAt;
  useEffect(() => {
    if (!startedAt) {
      setElapsed(0);
      return;
    }
    const startTime = new Date(startedAt).getTime();
    const tick = () => setElapsed(Math.floor((Date.now() - startTime) / 1000));
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, [startedAt]);

  const handleStartFocus = useCallback(async () => {
    await startFocus.mutate({ target_mins: 25 });
    refetch();
  }, [startFocus, refetch]);

  const handleStartPomodoro = useCallback(async () => {
    await startPomodoro.mutate({ work_mins: 25, break_mins: 5 });
    refetch();
  }, [startPomodoro, refetch]);

  const handleEnd = useCallback(async () => {
    await endFocus.mutate({});
    refetch();
  }, [endFocus, refetch]);

  const targetSecs = session?.targetMins ? session.targetMins * 60 : 0;
  const isPomodoro = session?.sessionType === "pomodoro";

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">
          {isPomodoro ? "Pomodoro Timer" : "Focus Session"}
        </h2>
        {session && (
          <span className="text-[10px] font-light text-dim">
            {session.interruptions} interruptions
          </span>
        )}
      </div>

      {session ? (
        <>
          <div className="flex items-center justify-between">
            <span className="text-[32px] font-light text-brand tabular-nums">
              {formatElapsed(elapsed)}
            </span>
            {targetSecs > 0 && (
              <div className="flex flex-col items-end gap-0.5">
                <span className="text-[10px] font-light text-dim">
                  Target: {session.targetMins}m
                </span>
                {session.qualityScore != null && (
                  <span className="text-[10px] font-light text-dim">
                    Quality:{" "}
                    <span className="tabular-nums">{Math.round(session.qualityScore * 100)}%</span>
                  </span>
                )}
              </div>
            )}
          </div>

          {targetSecs > 0 && (
            <div className="h-1.5 rounded-full bg-white/[0.08] overflow-hidden">
              <div
                className="h-full rounded-full bg-brand transition-[width]"
                style={{ width: `${Math.min((elapsed / targetSecs) * 100, 100)}%` }}
              />
            </div>
          )}

          <button
            type="button"
            onClick={handleEnd}
            disabled={endFocus.loading}
            className="flex items-center justify-center gap-2 py-2 rounded-lg bg-white/[0.08] text-destructive text-[12px] font-light hover:bg-white/[0.12] transition-colors"
          >
            <Square className="w-3.5 h-3.5" strokeWidth={1.5} />
            End Session
          </button>
        </>
      ) : (
        <div className="flex gap-2">
          <button
            type="button"
            onClick={handleStartFocus}
            disabled={startFocus.loading}
            className="flex-1 flex items-center justify-center gap-2 py-3 rounded-lg bg-brand text-white text-[13px] font-medium hover:bg-brand-hover transition-colors"
          >
            <Play className="w-4 h-4" strokeWidth={1.5} />
            Focus (25m)
          </button>
          <button
            type="button"
            onClick={handleStartPomodoro}
            disabled={startPomodoro.loading}
            className="flex items-center justify-center gap-2 px-4 py-3 rounded-lg bg-white/[0.08] text-secondary text-[13px] font-light hover:bg-white/[0.12] transition-colors"
          >
            <Timer className="w-4 h-4" strokeWidth={1.5} />
            Pomodoro
          </button>
        </div>
      )}
    </div>
  );
}
