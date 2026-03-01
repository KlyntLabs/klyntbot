import { useState, useEffect } from 'react';
import { Play, Pause, Plus, MessageSquare, Calendar, Settings } from 'lucide-react';
import { KlyntLogo } from '../ui/KlyntLogo';
import { Progress } from '../ui/Progress';
import { useQuery } from '../../hooks/useQuery';
import { mockCalendarEvents, mockTasks } from '../../data/mockData';
import type { CalendarEvent, AgentStatus as AgentStatusType } from '../../lib/types';

type DisplayStatus = 'active' | 'idle' | 'paused';

export function SystemTray() {
  const { data: agentStatusData } = useQuery<AgentStatusType>(
    'agent_status',
    undefined,
    { status: 'active', activeTaskCount: 3, focusTask: mockTasks.find(t => t.status === 'doing' && !t.completed) ?? null },
  );
  const { data: calendarEvents } = useQuery<CalendarEvent[]>(
    'calendar_events',
    { limit: 5 },
    mockCalendarEvents,
  );

  const [displayStatus, setDisplayStatus] = useState<DisplayStatus>('active');
  const [focusElapsed, setFocusElapsed] = useState(1260); // 21 min in seconds

  // Sync display status from IPC agent status
  useEffect(() => {
    if (agentStatusData.status === 'active' || agentStatusData.status === 'idle') {
      setDisplayStatus(agentStatusData.status as DisplayStatus);
    }
  }, [agentStatusData.status]);

  const focusTask = agentStatusData.focusTask;
  const focusDuration = 45 * 60; // 45 min focus session

  useEffect(() => {
    if (displayStatus !== 'active') return;
    const interval = setInterval(() => {
      setFocusElapsed(prev => Math.min(prev + 1, focusDuration));
    }, 1000);
    return () => clearInterval(interval);
  }, [displayStatus, focusDuration]);

  const formatTime = (seconds: number) => {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  const statusColors: Record<DisplayStatus, string> = {
    active: 'var(--color-success)',
    idle: 'var(--color-brand)',
    paused: 'var(--color-muted)',
  };

  const quickActions = [
    { icon: Plus, label: 'New Task' },
    { icon: MessageSquare, label: 'Chat' },
    { icon: Calendar, label: 'Calendar' },
    { icon: Settings, label: 'Settings' },
  ];

  return (
    <div className="h-screen w-screen bg-background text-primary flex items-start justify-center">
      <div className="w-[320px] rounded-2xl border border-border overflow-hidden bg-background shadow-2xl">
        {/* Header */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border">
          <div className="w-7 h-7 rounded-md bg-white flex items-center justify-center p-0.5">
            <KlyntLogo className="w-full h-full" />
          </div>
          <div className="flex-1">
            <p className="text-[13px] font-light text-primary">Klynt Agent</p>
            <div className="flex items-center gap-1.5">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: statusColors[displayStatus] }} />
              <span className="text-[11px] text-muted font-light capitalize">{displayStatus}</span>
            </div>
          </div>
          <button
            onClick={() => setDisplayStatus(prev => prev === 'active' ? 'paused' : 'active')}
            className="w-7 h-7 rounded-md bg-surface-base hover:bg-surface-highest flex items-center justify-center transition-colors"
          >
            {displayStatus === 'active' ? (
              <Pause className="w-3.5 h-3.5 text-muted" strokeWidth={1.5} />
            ) : (
              <Play className="w-3.5 h-3.5 text-success" strokeWidth={1.5} />
            )}
          </button>
        </div>

        {/* Focus Task */}
        {focusTask && (
          <div className="px-4 py-3 border-b border-border">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[11px] text-muted font-light uppercase tracking-wider">Focus</span>
              <span className="text-[12px] text-brand font-light font-mono">{formatTime(focusElapsed)}</span>
            </div>
            <p className="text-[13px] font-light text-secondary mb-2 truncate">{focusTask.title}</p>
            <Progress value={Math.round((focusElapsed / focusDuration) * 100)} />
            <div className="flex items-center justify-between mt-1.5">
              <span className="text-[10px] text-muted font-light">{Math.round((focusElapsed / focusDuration) * 100)}% of session</span>
              <span className="text-[10px] text-muted font-light">{formatTime(focusDuration - focusElapsed)} remaining</span>
            </div>
          </div>
        )}

        {/* Calendar Events */}
        <div className="px-4 py-3 border-b border-border">
          <span className="text-[11px] text-muted font-light uppercase tracking-wider">Today</span>
          <div className="mt-2 space-y-2">
            {calendarEvents.map(event => (
              <div key={event.id} className="flex items-center gap-2.5">
                <div className="w-1 h-6 rounded-full" style={{ backgroundColor: event.color }} />
                <div className="flex-1 min-w-0">
                  <p className="text-[12px] font-light text-secondary truncate">{event.title}</p>
                </div>
                <span className="text-[11px] text-muted font-light flex-shrink-0">{new Date(event.startAt).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Quick Actions */}
        <div className="grid grid-cols-4 gap-1 px-3 py-3">
          {quickActions.map(action => {
            const Icon = action.icon;
            return (
              <button
                key={action.label}
                className="flex flex-col items-center gap-1.5 py-2.5 rounded-lg hover:bg-surface-base transition-colors"
              >
                <div className="w-8 h-8 rounded-lg bg-surface-base flex items-center justify-center">
                  <Icon className="w-4 h-4 text-muted" strokeWidth={1.5} />
                </div>
                <span className="text-[10px] text-muted font-light">{action.label}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
