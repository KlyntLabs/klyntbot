import { useState, useMemo, useCallback } from 'react';
import { ChevronLeft, ChevronRight, RefreshCw, Loader2, X, CheckCircle2, Plus } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useNavigate } from 'react-router';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { CalendarEvent as ApiCalendarEvent, Task, CalendarSyncStatus, Project } from '../../lib/types';

type ViewMode = 'day' | 'week' | 'month';

type CalendarItem = {
  id: string;
  type: 'caldav' | 'task';
  title: string;
  description?: string;
  startTime: string; // HH:MM
  endTime?: string;
  date: string; // YYYY-MM-DD
  source?: string;
  status?: string;
  priority?: number;
  estimatedMinutes?: number;
  tags?: string[];
  project?: string;
  projectColor?: string;
  provider?: string;
};

export default function Calendar() {
  const navigate = useNavigate();
  const [viewMode, setViewMode] = useState<ViewMode>('month');
  const [selectedDate, setSelectedDate] = useState(new Date().toISOString().split('T')[0]);
  const [selectedItem, setSelectedItem] = useState<CalendarItem | null>(null);
  const [currentMonth, setCurrentMonth] = useState(new Date().getMonth() + 1);
  const [currentYear, setCurrentYear] = useState(new Date().getFullYear());

  const [syncing, setSyncing] = useState(false);
  const [creatingEvent, setCreatingEvent] = useState(false);
  const [eventForm, setEventForm] = useState({ summary: '', description: '', start: '', end: '' });
  const [eventSaving, setEventSaving] = useState(false);
  const [eventError, setEventError] = useState('');

  const { data: calendarEvents, refetch: refetchEvents } = useApi<ApiCalendarEvent[]>('/api/calendar/events');
  const { data: tasks, setData: setTasks, refetch: refetchTasks } = useApi<Task[]>('/api/tasks');
  const { data: syncStatus, refetch: refetchSyncStatus } = useApi<CalendarSyncStatus[]>('/api/calendar/sync-status');
  const { data: projects } = useApi<Project[]>('/api/projects');

  const handleSync = useCallback(async () => {
    setSyncing(true);
    try {
      await apiFetch('/api/calendar/sync', { method: 'POST' });
      refetchEvents();
      refetchSyncStatus();
    } catch {
      // Sync may have partially succeeded
    } finally {
      setSyncing(false);
    }
  }, [refetchEvents, refetchSyncStatus]);

  async function handleCreateEvent() {
    if (!eventForm.summary.trim()) { setEventError('Title is required'); return; }
    if (!eventForm.start || !eventForm.end) { setEventError('Start and end are required'); return; }
    setEventSaving(true);
    setEventError('');
    try {
      await apiFetch('/api/calendar/events', {
        body: {
          summary: eventForm.summary.trim(),
          description: eventForm.description.trim() || undefined,
          start: new Date(eventForm.start).toISOString(),
          end: new Date(eventForm.end).toISOString(),
        },
      });
      setCreatingEvent(false);
      setEventForm({ summary: '', description: '', start: '', end: '' });
      refetchEvents();
    } catch (e: any) {
      setEventError(e?.message || 'Failed to create event');
    } finally {
      setEventSaving(false);
    }
  }

  const handleMarkComplete = useCallback(async (taskId: string) => {
    setTasks(prev => prev?.map(t => t.id === taskId ? { ...t, status: 'done' } : t));
    setSelectedItem(prev => prev ? { ...prev, status: 'done' } : prev);
    try {
      await apiFetch(`/api/tasks/${encodeURIComponent(taskId)}`, {
        method: 'PATCH',
        body: { status: 'done' },
      });
      refetchTasks();
    } catch {
      refetchTasks();
    }
  }, [setTasks, refetchTasks]);

  const items = useMemo(() => {
    const result: CalendarItem[] = [];
    // Map CalDAV events
    if (calendarEvents) {
      for (const e of calendarEvents) {
        const startDate = new Date(e.startAt);
        const endDate = new Date(e.endAt);
        result.push({
          id: e.uid,
          type: 'caldav',
          title: e.summary,
          description: e.description ?? undefined,
          startTime: startDate.toTimeString().slice(0, 5),
          endTime: endDate.toTimeString().slice(0, 5),
          date: e.startAt.split('T')[0],
          source: e.source,
          status: e.status ?? undefined,
          provider: e.providerId,
        });
      }
    }
    // Map tasks with due dates
    if (tasks) {
      const projectMap = new Map((projects ?? []).map(p => [p.id, p]));
      for (const t of tasks) {
        if (!t.dueDate) continue;
        const proj = t.projectId ? projectMap.get(t.projectId) : undefined;
        result.push({
          id: t.id,
          type: 'task',
          title: t.title,
          description: t.description ?? undefined,
          startTime: '09:00', // tasks don't have specific times, default to morning
          date: t.dueDate.split('T')[0],
          status: t.status,
          priority: t.priority ?? undefined,
          estimatedMinutes: t.estimatedMinutes ?? undefined,
          tags: t.tags,
          project: proj?.name,
          projectColor: proj?.color,
        });
      }
    }
    return result;
  }, [calendarEvents, tasks, projects]);

  const getLastSyncLabel = (): string => {
    if (!syncStatus || syncStatus.length === 0) return 'Never synced';
    const latest = syncStatus
      .filter(s => s.lastSyncAt !== null)
      .sort((a, b) => new Date(b.lastSyncAt!).getTime() - new Date(a.lastSyncAt!).getTime())[0];
    if (!latest?.lastSyncAt) return 'Never synced';
    const diffMs = Date.now() - new Date(latest.lastSyncAt).getTime();
    const diffMin = Math.round(diffMs / 60000);
    if (diffMin < 1) return 'Last synced: just now';
    if (diffMin < 60) return `Last synced: ${diffMin} min ago`;
    const diffHr = Math.round(diffMin / 60);
    return `Last synced: ${diffHr}h ago`;
  };

  const getItemColor = (item: CalendarItem) => {
    if (item.type === 'caldav') return '#10a37f';
    if (item.date < selectedDate) return '#e55050';
    return '#d4a017';
  };

  const getItemsForDate = (date: string) => {
    return items.filter(e => e.date === date).sort((a, b) => a.startTime.localeCompare(b.startTime));
  };

  const getTodayItems = () => {
    return items.filter(e => e.date === selectedDate).sort((a, b) => a.startTime.localeCompare(b.startTime));
  };

  const formatDate = (dateStr: string) => {
    const d = new Date(dateStr);
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  };

  const getDaysInMonth = (month: number, year: number) => {
    return new Date(year, month, 0).getDate();
  };

  const getFirstDayOfMonth = (month: number, year: number) => {
    return new Date(year, month - 1, 1).getDay();
  };

  const monthNames = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];

  const renderMonthView = () => {
    const daysInMonth = getDaysInMonth(currentMonth, currentYear);
    const firstDay = getFirstDayOfMonth(currentMonth, currentYear);
    const days = [];

    for (let i = 0; i < firstDay; i++) {
      days.push(<div key={`empty-${i}`} className="p-2" style={{ minHeight: '120px' }} />);
    }

    for (let day = 1; day <= daysInMonth; day++) {
      const dateStr = `${currentYear}-${String(currentMonth).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
      const dayItems = getItemsForDate(dateStr);
      const isToday = dateStr === selectedDate;

      days.push(
        <div
          key={day}
          className="p-2 border cursor-pointer transition-colors"
          style={{
            borderColor: isToday ? '#10a37f' : '#1a1a1a',
            backgroundColor: isToday ? 'rgba(16, 163, 127, 0.05)' : 'transparent',
            minHeight: '120px',
            borderWidth: isToday ? '2px' : '1px'
          }}
          onClick={() => {
            setSelectedDate(dateStr);
            setViewMode('day');
          }}
          onMouseEnter={(e) => {
            if (!isToday) e.currentTarget.style.backgroundColor = '#141414';
          }}
          onMouseLeave={(e) => {
            if (!isToday) e.currentTarget.style.backgroundColor = 'transparent';
          }}
        >
          <div className="text-[13px] mb-2" style={{
            color: isToday ? '#10a37f' : 'var(--codex-fg-subtle)',
            fontWeight: isToday ? 500 : 400
          }}>
            {day}
          </div>
          <div className="space-y-1">
            {dayItems.slice(0, 3).map(item => (
              <div
                key={item.id}
                className="px-2 py-0.5 rounded text-[10px] truncate"
                style={{
                  backgroundColor: getItemColor(item) + '20',
                  color: getItemColor(item),
                  border: `1px solid ${getItemColor(item)}40`
                }}
                onClick={(e) => {
                  e.stopPropagation();
                  setSelectedItem(item);
                }}
              >
                {item.startTime} {item.title}
              </div>
            ))}
            {dayItems.length > 3 && (
              <div className="text-[10px] pl-2" style={{ color: '#666' }}>
                +{dayItems.length - 3} more
              </div>
            )}
          </div>
        </div>
      );
    }

    return (
      <div className="grid grid-cols-7 gap-0 border" style={{ borderColor: '#1a1a1a' }}>
        {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map(day => (
          <div
            key={day}
            className="py-3 text-center text-[11px] uppercase tracking-wider border-b"
            style={{
              color: 'var(--codex-fg-subtle)',
              backgroundColor: '#141414',
              borderColor: '#1a1a1a',
              fontWeight: 500
            }}
          >
            {day}
          </div>
        ))}
        {days}
      </div>
    );
  };

  const renderWeekView = () => {
    const hours = Array.from({ length: 17 }, (_, i) => i + 6);
    const weekDates = [];
    const selectedDateObj = new Date(selectedDate);
    const dayOfWeek = selectedDateObj.getDay();

    const sunday = new Date(selectedDateObj);
    sunday.setDate(selectedDateObj.getDate() - dayOfWeek);

    for (let i = 0; i < 7; i++) {
      const date = new Date(sunday);
      date.setDate(sunday.getDate() + i);
      weekDates.push(date.toISOString().split('T')[0]);
    }

    return (
      <div className="flex border" style={{ borderColor: '#1a1a1a' }}>
        <div className="w-16 border-r" style={{ borderColor: '#1a1a1a' }}>
          <div className="h-12 border-b" style={{ borderColor: '#1a1a1a' }} />
          {hours.map(hour => (
            <div
              key={hour}
              className="h-16 border-b flex items-start justify-end pr-2 pt-1 text-[11px]"
              style={{
                borderColor: '#1a1a1a',
                color: 'var(--codex-fg-subtle)',
                fontFamily: 'var(--font-mono)'
              }}
            >
              {hour > 12 ? hour - 12 : hour}:00 {hour >= 12 ? 'PM' : 'AM'}
            </div>
          ))}
        </div>

        {weekDates.map((date, idx) => {
          const dayItems = getItemsForDate(date);
          const dateObj = new Date(date);
          const isToday = date === selectedDate;

          return (
            <div key={date} className="flex-1 border-r" style={{ borderColor: '#1a1a1a' }}>
              <div
                className="h-12 border-b flex flex-col items-center justify-center"
                style={{
                  borderColor: '#1a1a1a',
                  backgroundColor: isToday ? 'rgba(16, 163, 127, 0.1)' : '#141414'
                }}
              >
                <div className="text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                  {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'][idx]}
                </div>
                <div
                  className="text-[14px]"
                  style={{
                    color: isToday ? '#10a37f' : 'var(--codex-fg)',
                    fontWeight: isToday ? 500 : 400
                  }}
                >
                  {dateObj.getDate()}
                </div>
              </div>

              <div className="relative">
                {hours.map(hour => (
                  <div
                    key={hour}
                    className="h-16 border-b"
                    style={{ borderColor: '#1a1a1a' }}
                  />
                ))}

                {dayItems.map(item => {
                  const [startHour, startMin] = item.startTime.split(':').map(Number);
                  const duration = item.estimatedMinutes || 60;
                  const top = ((startHour - 6) * 64) + (startMin / 60 * 64);
                  const height = (duration / 60) * 64;

                  return (
                    <div
                      key={item.id}
                      className="absolute left-1 right-1 px-2 py-1 rounded cursor-pointer overflow-hidden"
                      style={{
                        top: `${top}px`,
                        height: `${height}px`,
                        backgroundColor: getItemColor(item) + '30',
                        border: `1px solid ${getItemColor(item)}`,
                        zIndex: 1
                      }}
                      onClick={() => setSelectedItem(item)}
                    >
                      <div className="text-[11px] font-medium truncate" style={{ color: getItemColor(item) }}>
                        {item.title}
                      </div>
                      <div className="text-[10px]" style={{ color: getItemColor(item) + 'cc' }}>
                        {item.startTime}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    );
  };

  const renderDayView = () => {
    const hours = Array.from({ length: 17 }, (_, i) => i + 6);
    const dayItems = getItemsForDate(selectedDate);

    return (
      <div className="flex border" style={{ borderColor: '#1a1a1a' }}>
        <div className="w-20 border-r" style={{ borderColor: '#1a1a1a' }}>
          {hours.map(hour => (
            <div
              key={hour}
              className="h-20 border-b flex items-start justify-end pr-3 pt-2 text-[12px]"
              style={{
                borderColor: '#1a1a1a',
                color: 'var(--codex-fg-subtle)',
                fontFamily: 'var(--font-mono)'
              }}
            >
              {hour > 12 ? hour - 12 : hour}:00 {hour >= 12 ? 'PM' : 'AM'}
            </div>
          ))}
        </div>

        <div className="flex-1 relative">
          {hours.map(hour => (
            <div
              key={hour}
              className="h-20 border-b"
              style={{ borderColor: '#1a1a1a' }}
            />
          ))}

          {dayItems.map(item => {
            const [startHour, startMin] = item.startTime.split(':').map(Number);
            const duration = item.estimatedMinutes || 60;
            const top = ((startHour - 6) * 80) + (startMin / 60 * 80);
            const height = (duration / 60) * 80;

            return (
              <div
                key={item.id}
                className="absolute left-2 right-2 p-3 rounded cursor-pointer"
                style={{
                  top: `${top}px`,
                  height: `${height}px`,
                  backgroundColor: getItemColor(item) + '20',
                  border: `2px solid ${getItemColor(item)}`,
                  zIndex: 1
                }}
                onClick={() => setSelectedItem(item)}
              >
                <div className="flex items-start justify-between mb-1">
                  <div className="text-[13px] font-medium" style={{ color: getItemColor(item) }}>
                    {item.title}
                  </div>
                  {item.priority && (
                    <div className="w-2 h-2 rounded-full flex-shrink-0 ml-2" style={{
                      backgroundColor: item.priority === 1 ? '#e55050' : item.priority === 2 ? '#d4a017' : '#10a37f'
                    }} />
                  )}
                </div>
                <div className="text-[11px] mb-1" style={{
                  color: getItemColor(item) + 'cc',
                  fontFamily: 'var(--font-mono)'
                }}>
                  {item.startTime} - {item.endTime || 'TBD'}
                </div>
                {item.status && (
                  <div className="inline-block px-2 py-0.5 rounded text-[10px]" style={{
                    backgroundColor: '#141414',
                    color: getItemColor(item),
                    border: `1px solid ${getItemColor(item)}40`
                  }}>
                    {item.status}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  const getPriorityColor = (priority: number) => {
    if (priority === 1) return '#e55050';
    if (priority === 2) return '#d4a017';
    if (priority === 3) return '#10a37f';
    return '#666';
  };

  return (
    <>
      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <div className="border-b px-6 py-4 flex items-center justify-between" style={{
          borderColor: 'var(--codex-border-subtle)',
          backgroundColor: 'var(--codex-bg)'
        }}>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-3">
              <button
                onClick={() => {
                  if (currentMonth === 1) {
                    setCurrentMonth(12);
                    setCurrentYear(currentYear - 1);
                  } else {
                    setCurrentMonth(currentMonth - 1);
                  }
                }}
                className="p-1.5 rounded transition-colors"
                style={{ color: 'var(--codex-fg-subtle)' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#141414'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
              >
                <ChevronLeft className="w-4 h-4" strokeWidth={1.5} />
              </button>
              <h1 className="text-[16px] min-w-[160px] text-center" style={{
                color: 'var(--codex-fg)',
                fontWeight: 500
              }}>
                {monthNames[currentMonth - 1]} {currentYear}
              </h1>
              <button
                onClick={() => {
                  if (currentMonth === 12) {
                    setCurrentMonth(1);
                    setCurrentYear(currentYear + 1);
                  } else {
                    setCurrentMonth(currentMonth + 1);
                  }
                }}
                className="p-1.5 rounded transition-colors"
                style={{ color: 'var(--codex-fg-subtle)' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#141414'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
              >
                <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
              </button>
            </div>

            <button
              onClick={() => {
                const today = new Date().toISOString().split('T')[0];
                setSelectedDate(today);
                const now = new Date();
                setCurrentMonth(now.getMonth() + 1);
                setCurrentYear(now.getFullYear());
              }}
              className="px-3 py-1.5 rounded text-[13px] border transition-colors"
              style={{
                borderColor: 'var(--codex-border)',
                color: 'var(--codex-fg-subtle)'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = '#141414';
                e.currentTarget.style.color = 'var(--codex-fg)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = 'transparent';
                e.currentTarget.style.color = 'var(--codex-fg-subtle)';
              }}
            >
              Today
            </button>

            <div className="flex rounded-full p-1" style={{ backgroundColor: '#141414' }}>
              {(['day', 'week', 'month'] as ViewMode[]).map((mode) => (
                <button
                  key={mode}
                  onClick={() => setViewMode(mode)}
                  className="px-4 py-1.5 rounded-full text-[13px] transition-all capitalize"
                  style={{
                    backgroundColor: viewMode === mode ? '#10a37f' : 'transparent',
                    color: viewMode === mode ? 'white' : 'var(--codex-fg-subtle)'
                  }}
                >
                  {mode}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-3">
            <button
              onClick={() => setCreatingEvent(!creatingEvent)}
              className="px-4 py-2 rounded-lg text-[13px] flex items-center gap-2"
              style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
              onMouseEnter={e => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
              onMouseLeave={e => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
            >
              <Plus className="w-4 h-4" strokeWidth={1.5} />
              New Event
            </button>
            <button
              onClick={handleSync}
              disabled={syncing}
              className="flex items-center gap-2 text-[12px] px-3 py-1.5 rounded transition-colors"
              style={{ color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)' }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = '#141414'; e.currentTarget.style.color = '#10a37f'; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'var(--codex-fg-subtle)'; }}
            >
              {syncing || !syncStatus ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" strokeWidth={1.5} />
              ) : (
                <RefreshCw className="w-3.5 h-3.5" strokeWidth={1.5} />
              )}
              <span>{syncing ? 'Syncing...' : getLastSyncLabel()}</span>
            </button>
          </div>
        </div>

        {/* Create Event Form */}
        <AnimatePresence>
          {creatingEvent && (
            <motion.div initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }}
              className="border-b px-6 py-4"
              style={{ backgroundColor: '#141414', borderColor: 'var(--codex-border)' }}>
              <div className="max-w-2xl space-y-3">
                <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>New Event</h3>
                {/* Summary input */}
                <input type="text" placeholder="Event title..." value={eventForm.summary}
                  onChange={e => setEventForm(f => ({ ...f, summary: e.target.value }))}
                  className="w-full px-3 py-2 rounded-lg text-[13px] outline-none"
                  style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }}
                  onFocus={e => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                  onBlur={e => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                  autoFocus />
                {/* Description */}
                <input type="text" placeholder="Description (optional)" value={eventForm.description}
                  onChange={e => setEventForm(f => ({ ...f, description: e.target.value }))}
                  className="w-full px-3 py-2 rounded-lg text-[13px] outline-none"
                  style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }}
                  onFocus={e => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                  onBlur={e => e.currentTarget.style.borderColor = 'var(--codex-border)'} />
                {/* Start + End datetime row */}
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Start</label>
                    <input type="datetime-local" value={eventForm.start}
                      onChange={e => setEventForm(f => ({ ...f, start: e.target.value }))}
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none"
                      style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }} />
                  </div>
                  <div>
                    <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>End</label>
                    <input type="datetime-local" value={eventForm.end}
                      onChange={e => setEventForm(f => ({ ...f, end: e.target.value }))}
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none"
                      style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }} />
                  </div>
                </div>
                {/* Error */}
                {eventError && <div className="text-[12px]" style={{ color: '#ef4444' }}>{eventError}</div>}
                {/* Buttons */}
                <div className="flex justify-end gap-3">
                  <button onClick={() => { setCreatingEvent(false); setEventError(''); }}
                    className="px-4 py-2 rounded-lg text-[13px]"
                    style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}>
                    Cancel
                  </button>
                  <button onClick={handleCreateEvent} disabled={eventSaving}
                    className="px-4 py-2 rounded-lg text-[13px] flex items-center gap-2"
                    style={{ backgroundColor: 'var(--codex-accent)', color: 'white', opacity: eventSaving ? 0.7 : 1 }}>
                    {eventSaving && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                    Create Event
                  </button>
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Calendar Grid */}
        <div className="flex-1 overflow-auto p-6" style={{ backgroundColor: 'var(--codex-bg)' }}>
          {viewMode === 'month' && renderMonthView()}
          {viewMode === 'week' && renderWeekView()}
          {viewMode === 'day' && renderDayView()}
        </div>

        {/* Legend */}
        <div className="border-t px-6 py-3 flex items-center gap-6" style={{
          borderColor: 'var(--codex-border-subtle)',
          backgroundColor: 'var(--codex-bg)'
        }}>
          <div className="flex items-center gap-2 text-[12px]">
            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#10a37f' }} />
            <span style={{ color: 'var(--codex-fg-subtle)' }}>CalDAV Events</span>
          </div>
          <div className="flex items-center gap-2 text-[12px]">
            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#d4a017' }} />
            <span style={{ color: 'var(--codex-fg-subtle)' }}>Tasks</span>
          </div>
          <div className="flex items-center gap-2 text-[12px]">
            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#e55050' }} />
            <span style={{ color: 'var(--codex-fg-subtle)' }}>Overdue</span>
          </div>
        </div>
      </div>

      {/* Right Sidebar */}
      <aside className="w-[280px] border-l overflow-y-auto" style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)'
      }}>
        {selectedItem ? (
          <div className="p-4">
            <div className="flex items-start justify-between mb-4">
              <h3 className="text-[14px]" style={{
                color: 'var(--codex-fg)',
                fontWeight: 500
              }}>
                Details
              </h3>
              <button
                onClick={() => setSelectedItem(null)}
                className="p-1 rounded transition-colors"
                style={{ color: 'var(--codex-fg-subtle)' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#141414'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
              >
                <X className="w-4 h-4" strokeWidth={1.5} />
              </button>
            </div>

            <div className="space-y-4">
              <div>
                <div className="text-[16px] mb-2" style={{ color: 'var(--codex-fg)' }}>
                  {selectedItem.title}
                </div>
                {selectedItem.description && (
                  <div className="text-[13px]" style={{ color: 'var(--codex-fg-muted)' }}>
                    {selectedItem.description}
                  </div>
                )}
              </div>

              <div className="flex items-center justify-between text-[13px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Time</span>
                <span style={{
                  color: 'var(--codex-fg)',
                  fontFamily: 'var(--font-mono)'
                }}>
                  {selectedItem.startTime} {selectedItem.endTime && `- ${selectedItem.endTime}`}
                </span>
              </div>

              <div className="flex items-center justify-between text-[13px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Date</span>
                <span style={{ color: 'var(--codex-fg)' }}>
                  {formatDate(selectedItem.date)}
                </span>
              </div>

              {selectedItem.type === 'task' ? (
                <>
                  {selectedItem.priority && (
                    <div className="flex items-center justify-between text-[13px]">
                      <span style={{ color: 'var(--codex-fg-subtle)' }}>Priority</span>
                      <div className="flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full" style={{
                          backgroundColor: getPriorityColor(selectedItem.priority)
                        }} />
                        <span style={{ color: 'var(--codex-fg)' }}>P{selectedItem.priority}</span>
                      </div>
                    </div>
                  )}

                  <div className="flex items-center justify-between text-[13px]">
                    <span style={{ color: 'var(--codex-fg-subtle)' }}>Status</span>
                    <span className="px-2 py-1 rounded text-[11px]" style={{
                      backgroundColor: '#141414',
                      color: getItemColor(selectedItem),
                      border: `1px solid ${getItemColor(selectedItem)}40`
                    }}>
                      {selectedItem.status}
                    </span>
                  </div>

                  {selectedItem.estimatedMinutes && (
                    <div className="flex items-center justify-between text-[13px]">
                      <span style={{ color: 'var(--codex-fg-subtle)' }}>Estimated</span>
                      <span style={{ color: 'var(--codex-fg)' }}>{selectedItem.estimatedMinutes}m</span>
                    </div>
                  )}

                  {selectedItem.tags && selectedItem.tags.length > 0 && (
                    <div>
                      <div className="text-[12px] mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>Tags</div>
                      <div className="flex flex-wrap gap-1.5">
                        {selectedItem.tags.map(tag => (
                          <span
                            key={tag}
                            className="px-2 py-0.5 rounded text-[11px]"
                            style={{
                              backgroundColor: '#141414',
                              color: 'var(--codex-fg-subtle)',
                              border: '1px solid var(--codex-border)'
                            }}
                          >
                            {tag}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}

                  {selectedItem.project && (
                    <div className="flex items-center justify-between text-[13px]">
                      <span style={{ color: 'var(--codex-fg-subtle)' }}>Project</span>
                      <div className="flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full" style={{
                          backgroundColor: selectedItem.projectColor
                        }} />
                        <span style={{ color: 'var(--codex-fg)' }}>{selectedItem.project}</span>
                      </div>
                    </div>
                  )}
                </>
              ) : (
                <>
                  <div className="flex items-center justify-between text-[13px]">
                    <span style={{ color: 'var(--codex-fg-subtle)' }}>Source</span>
                    <span className="px-2 py-1 rounded text-[11px]" style={{
                      backgroundColor: '#141414',
                      color: '#10a37f',
                      border: '1px solid #10a37f40'
                    }}>
                      {selectedItem.source}
                    </span>
                  </div>

                  {selectedItem.provider && (
                    <div className="flex items-center justify-between text-[13px]">
                      <span style={{ color: 'var(--codex-fg-subtle)' }}>Provider</span>
                      <span className="px-2 py-1 rounded text-[11px] capitalize" style={{
                        backgroundColor: '#141414',
                        color: 'var(--codex-fg-subtle)',
                        border: '1px solid var(--codex-border)'
                      }}>
                        {selectedItem.provider}
                      </span>
                    </div>
                  )}

                  <div className="flex items-center justify-between text-[13px]">
                    <span style={{ color: 'var(--codex-fg-subtle)' }}>Status</span>
                    <span className="px-2 py-1 rounded text-[11px]" style={{
                      backgroundColor: '#141414',
                      color: '#10a37f',
                      border: '1px solid #10a37f40'
                    }}>
                      {selectedItem.status}
                    </span>
                  </div>
                </>
              )}
            </div>

            {selectedItem.type === 'task' && (
              <div className="mt-4 space-y-2">
                {selectedItem.status?.toLowerCase() !== 'done' && (
                  <button
                    onClick={() => handleMarkComplete(selectedItem.id)}
                    className="w-full flex items-center justify-center gap-2 px-4 py-2 rounded text-[13px] transition-colors"
                    style={{
                      backgroundColor: 'rgba(16, 163, 127, 0.15)',
                      color: '#10a37f',
                      border: '1px solid #10a37f40'
                    }}
                    onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'rgba(16, 163, 127, 0.25)'}
                    onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'rgba(16, 163, 127, 0.15)'}
                  >
                    <CheckCircle2 className="w-4 h-4" strokeWidth={1.5} />
                    Mark Complete
                  </button>
                )}
                <button
                  onClick={() => navigate(`/tasks/${selectedItem.id}`)}
                  className="w-full px-4 py-2 rounded text-[13px] border transition-colors"
                  style={{
                    borderColor: 'var(--codex-border)',
                    color: 'var(--codex-fg-subtle)',
                    backgroundColor: 'transparent'
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.backgroundColor = '#141414';
                    e.currentTarget.style.color = 'var(--codex-fg)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.backgroundColor = 'transparent';
                    e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                  }}
                >
                  View in Tasks
                </button>
              </div>
            )}
          </div>
        ) : (
          <div className="p-4">
            <h3 className="text-[14px] mb-4" style={{
              color: 'var(--codex-fg)',
              fontWeight: 500
            }}>
              Today's Schedule
            </h3>
            <div className="space-y-3">
              {getTodayItems().length === 0 ? (
                <div className="text-[13px] text-center py-8" style={{ color: 'var(--codex-fg-subtle)' }}>
                  No events or tasks today
                </div>
              ) : (
                getTodayItems().map(item => (
                  <div
                    key={item.id}
                    className="p-3 rounded-lg cursor-pointer transition-colors"
                    style={{
                      backgroundColor: '#141414',
                      border: `1px solid ${getItemColor(item)}40`
                    }}
                    onClick={() => setSelectedItem(item)}
                    onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
                    onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#141414'}
                  >
                    <div className="flex items-start justify-between mb-2">
                      <div className="text-[13px] font-medium" style={{ color: 'var(--codex-fg)' }}>
                        {item.title}
                      </div>
                      {item.priority && (
                        <div className="w-2 h-2 rounded-full flex-shrink-0 ml-2" style={{
                          backgroundColor: getPriorityColor(item.priority)
                        }} />
                      )}
                    </div>
                    <div className="text-[11px] mb-2" style={{
                      color: 'var(--codex-fg-subtle)',
                      fontFamily: 'var(--font-mono)'
                    }}>
                      {item.startTime} {item.endTime && `- ${item.endTime}`}
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="px-2 py-0.5 rounded text-[10px]" style={{
                        backgroundColor: getItemColor(item) + '20',
                        color: getItemColor(item),
                        border: `1px solid ${getItemColor(item)}40`
                      }}>
                        {item.type === 'caldav' ? item.source : 'Task'}
                      </span>
                      {item.status && (
                        <span className="px-2 py-0.5 rounded text-[10px]" style={{
                          backgroundColor: '#0d0d0d',
                          color: 'var(--codex-fg-subtle)',
                          border: '1px solid var(--codex-border)'
                        }}>
                          {item.status}
                        </span>
                      )}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </aside>
    </>
  );
}
