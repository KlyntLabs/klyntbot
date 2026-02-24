import { useState } from 'react';
import { ChevronLeft, ChevronRight, RefreshCw, X } from 'lucide-react';
import { useNavigate } from 'react-router';

type ViewMode = 'day' | 'week' | 'month';

type CalendarEvent = {
  id: string;
  type: 'caldav' | 'task';
  title: string;
  description?: string;
  startTime: string;
  endTime?: string;
  date: string;
  source?: string;
  status?: string;
  priority?: number;
  estimatedMinutes?: number;
  tags?: string[];
  project?: string;
  projectColor?: string;
  subtaskCount?: number;
  dependencyCount?: number;
  timeTracked?: number;
  attachments?: number;
  provider?: 'apple' | 'google';
  etag?: string;
  blocked_by?: string;
};

export default function Calendar() {
  const navigate = useNavigate();
  const [viewMode, setViewMode] = useState<ViewMode>('month');
  const [selectedDate, setSelectedDate] = useState('2026-02-24');
  const [selectedItem, setSelectedItem] = useState<CalendarEvent | null>(null);
  const [currentMonth, setCurrentMonth] = useState(2);
  const [currentYear, setCurrentYear] = useState(2026);

  const events: CalendarEvent[] = [
    { id: '1', type: 'caldav', title: 'Team Standup', startTime: '09:00', endTime: '09:30', date: '2026-02-24', source: 'CalDAV', status: 'CONFIRMED', provider: 'google' },
    { id: '2', type: 'task', title: 'Fix auth token refresh', startTime: '10:00', date: '2026-02-24', priority: 1, status: 'Doing', estimatedMinutes: 60, tags: ['backend', 'security'], project: 'Backend', projectColor: '#4a9eff', timeTracked: 45, blocked_by: 'Review PR #142' },
    { id: '3', type: 'caldav', title: 'Team Standup', startTime: '09:00', endTime: '09:30', date: '2026-02-25', source: 'CalDAV', status: 'CONFIRMED', provider: 'google' },
    { id: '4', type: 'task', title: 'Update API docs', startTime: '14:00', date: '2026-02-25', priority: 3, status: 'Todo', estimatedMinutes: 30, tags: ['docs'], project: 'Documentation', projectColor: '#d4a017' },
    { id: '5', type: 'caldav', title: 'Team Standup', startTime: '09:00', endTime: '09:30', date: '2026-02-26', source: 'CalDAV', status: 'CONFIRMED', provider: 'google' },
    { id: '6', type: 'task', title: 'Refactor storage layer', startTime: '11:00', date: '2026-02-26', priority: 2, status: 'Todo', estimatedMinutes: 120, tags: ['backend', 'tech-debt'], project: 'Backend', projectColor: '#4a9eff' },
    { id: '7', type: 'caldav', title: 'Team Standup', startTime: '09:00', endTime: '09:30', date: '2026-02-27', source: 'CalDAV', status: 'CONFIRMED', provider: 'google' },
    { id: '8', type: 'caldav', title: 'Sprint Review', startTime: '14:00', endTime: '15:00', date: '2026-02-27', source: 'CalDAV', status: 'CONFIRMED', provider: 'google' },
    { id: '9', type: 'task', title: 'Review PR #142', startTime: '16:00', date: '2026-02-22', priority: 2, status: 'Todo', estimatedMinutes: 15, tags: ['review'] },
  ];

  const getItemColor = (item: CalendarEvent) => {
    if (item.type === 'caldav') return '#10a37f';
    if (item.date < selectedDate) return '#e55050';
    return '#d4a017';
  };

  const getItemsForDate = (date: string) => {
    return events.filter(e => e.date === date).sort((a, b) => a.startTime.localeCompare(b.startTime));
  };

  const getTodayItems = () => {
    return events.filter(e => e.date === selectedDate).sort((a, b) => a.startTime.localeCompare(b.startTime));
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
              onClick={() => setSelectedDate('2026-02-24')}
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

          <div className="flex items-center gap-2 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            <RefreshCw className="w-3.5 h-3.5" strokeWidth={1.5} />
            <span>Last synced: 2 min ago</span>
          </div>
        </div>

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

                  {selectedItem.timeTracked !== undefined && selectedItem.timeTracked > 0 && (
                    <div className="flex items-center justify-between text-[13px]">
                      <span style={{ color: 'var(--codex-fg-subtle)' }}>Time Tracked</span>
                      <span style={{ color: '#10a37f' }}>{selectedItem.timeTracked}m</span>
                    </div>
                  )}

                  {selectedItem.blocked_by && (
                    <div className="flex items-center justify-between text-[13px]">
                      <span style={{ color: 'var(--codex-fg-subtle)' }}>Blocked By</span>
                      <span style={{ color: '#e55050' }}>{selectedItem.blocked_by}</span>
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
              <button
                onClick={() => navigate(`/tasks/${selectedItem.id}`)}
                className="w-full mt-4 px-4 py-2 rounded text-[13px] border transition-colors"
                style={{
                  borderColor: '#10a37f',
                  color: '#10a37f',
                  backgroundColor: 'transparent'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = 'rgba(16, 163, 127, 0.1)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'transparent';
                }}
              >
                View in Tasks
              </button>
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
