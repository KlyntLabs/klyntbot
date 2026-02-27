import { useState } from 'react';
import { Calendar } from 'lucide-react';
import { useApi } from '../../../lib/hooks/useApi';
import type { CalendarEvent } from '../../../lib/types';
import { formatRelativeTime } from '../utils';
import { SidebarSection } from './SidebarSection';

export function UpcomingEvents() {
  const [open, setOpen] = useState(true);
  const { data: calendarEvents } = useApi<CalendarEvent[]>('/api/calendar/events', {
    params: { limit: 5 },
  });

  return (
    <SidebarSection title="Upcoming" open={open} onToggle={() => setOpen(!open)} noBorder>
      <div className="px-4 pb-4 space-y-2">
        {(!calendarEvents || calendarEvents.length === 0) && (
          <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            No upcoming events
          </div>
        )}
        {calendarEvents?.slice(0, 5).map((event) => (
          <div
            key={event.uid}
            className="flex items-center gap-2 p-1.5 rounded"
            style={{ backgroundColor: 'transparent' }}
            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
          >
            <Calendar
              className="w-3 h-3 flex-shrink-0"
              strokeWidth={1.5}
              style={{ color: 'var(--codex-accent)' }}
            />
            <span className="text-[12px] truncate flex-1" style={{ color: 'var(--codex-fg)' }}>
              {event.summary}
            </span>
            <span
              className="text-[10px] flex-shrink-0"
              style={{ color: 'var(--codex-fg-subtle)', fontFamily: 'var(--font-mono)' }}
            >
              {formatRelativeTime(event.startAt)}
            </span>
          </div>
        ))}
      </div>
    </SidebarSection>
  );
}
