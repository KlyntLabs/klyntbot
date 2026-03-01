import { useState, useEffect } from 'react';
import { Play, Pause, Plus, MessageSquare, Calendar, Settings } from 'lucide-react';
import { KlyntLogo } from '../ui/KlyntLogo';
import { Progress } from '../ui/Progress';
import { mockCalendarEvents, mockTasks } from '../../data/mockData';

type AgentStatus = 'active' | 'idle' | 'paused';

export function SystemTray() {
  const [agentStatus, setAgentStatus] = useState<AgentStatus>('active');
  const [focusElapsed, setFocusElapsed] = useState(1260); // 21 min in seconds

  const focusTask = mockTasks.find(t => t.status === 'Doing' && !t.completed);
  const focusDuration = 45 * 60; // 45 min focus session

  useEffect(() => {
    if (agentStatus !== 'active') return;
    const interval = setInterval(() => {
      setFocusElapsed(prev => Math.min(prev + 1, focusDuration));
    }, 1000);
    return () => clearInterval(interval);
  }, [agentStatus, focusDuration]);

  const formatTime = (seconds: number) => {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  const statusColors: Record<AgentStatus, string> = {
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
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: statusColors[agentStatus] }} />
              <span className="text-[11px] text-muted font-light capitalize">{agentStatus}</span>
            </div>
          </div>
          <button
            onClick={() => setAgentStatus(prev => prev === 'active' ? 'paused' : 'active')}
            className="w-7 h-7 rounded-md bg-surface-base hover:bg-surface-highest flex items-center justify-center transition-colors"
          >
            {agentStatus === 'active' ? (
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
            {mockCalendarEvents.map(event => (
              <div key={event.id} className="flex items-center gap-2.5">
                <div className="w-1 h-6 rounded-full" style={{ backgroundColor: event.color }} />
                <div className="flex-1 min-w-0">
                  <p className="text-[12px] font-light text-secondary truncate">{event.title}</p>
                </div>
                <span className="text-[11px] text-muted font-light flex-shrink-0">{event.time}</span>
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
