import { useState, useEffect, useCallback } from 'react';
import { Play, Square } from 'lucide-react';
import { useQuery } from '../../hooks/useQuery';
import { useMutation } from '../../hooks/useMutation';
import { useEvent } from '../../hooks/useEvent';
import { formatElapsed } from '../../lib/dates';
import type { FocusSession } from '../../lib/types';

const DEFAULT_FOCUS_MINS = 25;

export function FocusStatusCard() {
  const { data: session, refetch } = useQuery<FocusSession | null>('productivity_focus_status', undefined, null);
  const startFocus = useMutation<FocusSession, { target_mins?: number }>('productivity_focus_start');
  const endFocus = useMutation<FocusSession | null, { notes?: string }>('productivity_focus_end');

  const [elapsed, setElapsed] = useState(0);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    if (payload?.entityKind === 'focus_session') refetch();
  });

  // Live timer
  useEffect(() => {
    if (!session) { setElapsed(0); return; }

    const startTime = new Date(session.startedAt).getTime();
    const tick = () => setElapsed(Math.floor((Date.now() - startTime) / 1000));
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, [session]);

  const handleStart = useCallback(async () => {
    await startFocus.mutate({ target_mins: DEFAULT_FOCUS_MINS });
    refetch();
  }, [startFocus, refetch]);

  const handleEnd = useCallback(async () => {
    await endFocus.mutate({});
    refetch();
  }, [endFocus, refetch]);

  const targetSecs = session?.targetMins ? session.targetMins * 60 : 0;
  const progress = targetSecs > 0 ? Math.min(elapsed / targetSecs, 1) : 0;

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Focus Session</h2>

      {session ? (
        <>
          <div className="flex items-center justify-between">
            <div className="flex flex-col gap-1">
              <span className="text-[28px] font-light text-brand tabular-nums">
                {formatElapsed(elapsed)}
              </span>
              {targetSecs > 0 && (
                <span className="text-[11px] font-light text-dim">
                  Target: {session.targetMins}m
                </span>
              )}
            </div>
            <div className="flex items-center gap-3 text-[11px] font-light text-muted">
              <span>{session.interruptions} interruptions</span>
              {session.qualityScore != null && (
                <span>Quality: {Math.round(session.qualityScore * 100)}%</span>
              )}
            </div>
          </div>

          {/* Progress bar */}
          {targetSecs > 0 && (
            <div className="h-1.5 rounded-full bg-surface-raised overflow-hidden">
              <div
                className="h-full rounded-full bg-brand transition-all"
                style={{ width: `${progress * 100}%` }}
              />
            </div>
          )}

          <button
            onClick={handleEnd}
            disabled={endFocus.loading}
            className="flex items-center justify-center gap-2 py-2 rounded-lg bg-surface-raised text-destructive text-[12px] font-light hover:bg-surface-highest transition-colors"
          >
            <Square className="w-3.5 h-3.5" strokeWidth={1.5} />
            End Focus
          </button>
        </>
      ) : (
        <button
          onClick={handleStart}
          disabled={startFocus.loading}
          className="flex items-center justify-center gap-2 py-3 rounded-lg bg-brand text-white text-[13px] font-medium hover:bg-brand-hover transition-colors"
        >
          <Play className="w-4 h-4" strokeWidth={1.5} />
          Start Focus Session
        </button>
      )}
    </div>
  );
}
