import { useState, useCallback, useEffect, useRef } from 'react';
import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { Sidebar } from '../layout/Sidebar';
import { FocusStatusCard } from '../productivity/FocusStatusCard';
import { TodaySummary } from '../productivity/TodaySummary';
import { Timeline } from '../productivity/Timeline';
import { TopApps } from '../productivity/TopApps';
import { WeeklyTrend } from '../productivity/WeeklyTrend';
import { FocusSessionsList } from '../productivity/FocusSessionsList';
import type { ProductivitySummary } from '../../lib/types';

interface Toast {
  id: number;
  message: string;
  variant: 'warning' | 'info';
}

const MAX_TOASTS = 5;

export function Productivity() {
  const today = new Date().toISOString().slice(0, 10);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastIdRef = useRef(0);
  const timersRef = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map());

  const { data: summary, refetch } = useQuery<ProductivitySummary | null>('productivity_today', undefined, null);

  useEvent<{ entityKind: string }>('entity:updated', (payload) => {
    const k = payload?.entityKind;
    if (k === 'productivity' || k === 'focus_session') refetch();
  });

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    timersRef.current.delete(id);
  }, []);

  const addToast = useCallback((message: string, variant: Toast['variant']) => {
    const id = ++toastIdRef.current;
    setToasts((prev) => [...prev.slice(-(MAX_TOASTS - 1)), { id, message, variant }]);
    timersRef.current.set(id, setTimeout(() => dismissToast(id), 5000));
  }, [dismissToast]);

  // Clean up timers on unmount
  useEffect(() => {
    return () => timersRef.current.forEach((t) => clearTimeout(t));
  }, []);

  useEvent<{ appName: string }>('productivity:distraction', (payload) => {
    if (payload?.appName) {
      addToast(`Distraction detected: ${payload.appName} — you're in focus mode!`, 'warning');
    }
  });

  useEvent<{ message: string }>('productivity:nudge', (payload) => {
    if (payload?.message) {
      addToast(payload.message, 'info');
    }
  });

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar active="Productivity" />
      <div className="flex-1 flex flex-col overflow-hidden">
        <div className="h-14 bg-background flex items-center px-4">
          <h1 className="text-lg font-semibold text-primary">Productivity</h1>
        </div>
        <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
          {toasts.map((t) => (
            <div
              key={t.id}
              className={`rounded-lg px-4 py-2.5 text-[12px] font-medium animate-in fade-in slide-in-from-top-2 ${
                t.variant === 'warning'
                  ? 'bg-destructive/15 text-destructive border border-destructive/20'
                  : 'bg-info/15 text-info border border-info/20'
              }`}
            >
              {t.message}
            </div>
          ))}
          <FocusStatusCard />
          <TodaySummary summary={summary} />
          <Timeline date={today} />
          <div className="grid grid-cols-2 gap-4">
            <FocusSessionsList date={today} />
            <TopApps apps={summary?.topApps ?? []} />
          </div>
          <WeeklyTrend />
        </div>
      </div>
    </div>
  );
}
