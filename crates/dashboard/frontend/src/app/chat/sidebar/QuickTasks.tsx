import { useState, useMemo } from 'react';
import { Circle } from 'lucide-react';
import { useApi } from '../../../lib/hooks/useApi';
import type { Task } from '../../../lib/types';
import { priorityDisplay } from '../utils';
import { SidebarSection } from './SidebarSection';

export function QuickTasks() {
  const [open, setOpen] = useState(true);
  const { data: tasks } = useApi<Task[]>('/api/tasks');

  const pendingTasks = useMemo(() => {
    if (!tasks) return [];
    return tasks
      .filter((t) => t.status === 'todo' || t.status === 'doing')
      .sort((a, b) => (a.priority ?? 99) - (b.priority ?? 99))
      .slice(0, 5);
  }, [tasks]);

  return (
    <SidebarSection title="Quick Tasks" open={open} onToggle={() => setOpen(!open)}>
      <div className="px-4 pb-4 space-y-2">
        {pendingTasks.length === 0 && (
          <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            No pending tasks
          </div>
        )}
        {pendingTasks.map((task) => {
          const p = priorityDisplay(task.priority);
          return (
            <div
              key={task.id}
              className="flex items-center gap-2 p-1.5 rounded cursor-default"
              style={{ backgroundColor: 'transparent' }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
            >
              <Circle
                className="w-3 h-3 flex-shrink-0"
                strokeWidth={1.5}
                style={{
                  color: task.status === 'doing' ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
                }}
              />
              <span className="text-[12px] truncate flex-1" style={{ color: 'var(--codex-fg)' }}>
                {task.title}
              </span>
              <span
                className="text-[10px] flex-shrink-0"
                style={{ color: p.color, fontFamily: 'var(--font-mono)', fontWeight: 500 }}
              >
                {p.label}
              </span>
            </div>
          );
        })}
      </div>
    </SidebarSection>
  );
}
