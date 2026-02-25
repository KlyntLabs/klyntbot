import { useState, useMemo } from 'react';
import { useNavigate } from 'react-router';
import {
  Plus,
  Search,
  Filter,
  Clock,
  Calendar as CalendarIcon,
  Paperclip,
  ChevronDown,
  Zap,
  Link,
  RefreshCw,
  Timer,
  Circle,
  CheckCircle2,
  Archive,
  Loader2,
  AlertTriangle,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import type { Task, TaskSummary, Project } from '../../lib/types';

export default function Tasks() {
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterOpen, setFilterOpen] = useState(false);

  const { data: tasks, loading, error } = useApi<Task[]>('/api/tasks');
  const { data: summary } = useApi<TaskSummary>('/api/tasks/summary');
  const { data: projects } = useApi<Project[]>('/api/projects');

  // Build project lookup map for name/color display
  const projectMap = useMemo(() => {
    const map = new Map<string, Project>();
    if (projects) {
      for (const p of projects) map.set(p.id, p);
    }
    return map;
  }, [projects]);

  const getPriorityColor = (priority: number | null) => {
    switch (priority) {
      case 1: return '#e55050';
      case 2: return '#d4a017';
      case 3: return '#e5c07b';
      case 4: return '#666';
      default: return '#666';
    }
  };

  const formatTime = (minutes: number) => {
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  };

  const formatTrackedTime = (seconds: number) => formatTime(Math.floor(seconds / 60));

  const today = new Date().toISOString().split('T')[0];
  const isOverdue = (dueDate: string | null) => dueDate ? dueDate < today : false;

  const formatDate = (dateStr: string) =>
    new Date(dateStr).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });

  const getStatusIcon = (status: string) => {
    switch (status.toLowerCase()) {
      case 'todo': return <Circle className="w-5 h-5" strokeWidth={1.5} style={{ color: '#666' }} />;
      case 'doing': return <Circle className="w-5 h-5 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      case 'done': return <CheckCircle2 className="w-5 h-5 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      case 'archived': return <Archive className="w-5 h-5" strokeWidth={1.5} style={{ color: '#666' }} />;
      default: return <Circle className="w-5 h-5" strokeWidth={1.5} style={{ color: '#666' }} />;
    }
  };

  const taskList = tasks ?? [];
  const activeTasks = taskList.filter(t => { const s = t.status.toLowerCase(); return s === 'todo' || s === 'doing'; });
  const overdueTasks = taskList.filter(t => isOverdue(t.dueDate) && t.status.toLowerCase() !== 'done');
  const completedTasks = taskList.filter(t => t.status.toLowerCase() === 'done');
  const doingTasks = taskList.filter(t => t.status.toLowerCase() === 'doing');
  const focusedTasks = taskList.filter(t => t.focusedAt);

  if (loading && !tasks) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <Loader2 className="w-5 h-5 animate-spin" style={{ color: 'var(--codex-fg-subtle)' }} />
      </div>
    );
  }

  if (error && !tasks) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2" style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg-subtle)' }}>
        <AlertTriangle className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px]">Failed to load tasks</span>
      </div>
    );
  }

  return (
    <>
      {/* Center Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <div className="border-b px-6 py-4" style={{ borderColor: 'var(--codex-border-subtle)', backgroundColor: 'var(--codex-bg)' }}>
          <div className="flex items-center justify-between mb-4">
            <h1 className="text-xl" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>Tasks</h1>
            {/* TODO: wire to POST /api/tasks */}
            <button
              className="flex items-center gap-2 px-4 py-2 rounded-lg text-[13px] transition-colors"
              style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
            >
              <Plus className="w-4 h-4" strokeWidth={1.5} />
              New Task
            </button>
          </div>

          {/* Search and Filter */}
          <div className="flex items-center gap-3 mb-4">
            <div className="flex-1 relative">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
              {/* TODO: wire search to API query params */}
              <input
                type="text" placeholder="Search tasks..." value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2 rounded-lg border outline-none text-[13px]"
                style={{ backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)', color: 'var(--codex-fg)' }}
              />
            </div>
            <div className="relative">
              {/* TODO: wire filter to API query params (status, priority, projectId, tags) */}
              <button
                onClick={() => setFilterOpen(!filterOpen)}
                className="flex items-center gap-2 px-4 py-2 rounded-lg border text-[13px] transition-colors"
                style={{ backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)', color: 'var(--codex-fg-subtle)' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg-tertiary)'}
              >
                <Filter className="w-4 h-4" strokeWidth={1.5} />
                Filter
              </button>
              <AnimatePresence>
                {filterOpen && (
                  <motion.div
                    initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }}
                    className="absolute right-0 mt-2 p-4 rounded-lg w-64 z-10"
                    style={{ backgroundColor: 'var(--codex-bg-tertiary)', border: '1px solid var(--codex-border)' }}
                  >
                    <div className="space-y-4">
                      <div>
                        <div className="text-[12px] mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Status</div>
                        <div className="space-y-2">
                          {['Todo', 'Doing', 'Done', 'Archived'].map(status => (
                            <label key={status} className="flex items-center gap-2 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                              <input type="checkbox" defaultChecked /> {status}
                            </label>
                          ))}
                        </div>
                      </div>
                      <div>
                        <div className="text-[12px] mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Priority</div>
                        <div className="space-y-2">
                          {([1, 2, 3, 4] as number[]).map(p => (
                            <label key={p} className="flex items-center gap-2 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                              <input type="checkbox" defaultChecked />
                              <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(p) }} />
                              P{p}
                            </label>
                          ))}
                        </div>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>

          {/* Summary — use server-side counts when available, fall back to client-side */}
          <div className="flex items-center gap-2 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            <span>{summary ? summary.todo + summary.doing : activeTasks.length} Active</span><span>&middot;</span>
            <span style={{ color: overdueTasks.length > 0 ? '#e55050' : 'var(--codex-fg-subtle)' }}>{overdueTasks.length} Overdue</span>
            <span>&middot;</span><span>{summary?.done ?? completedTasks.length} Completed</span>
            <span>&middot;</span><span style={{ color: '#10a37f' }}>{summary?.doing ?? doingTasks.length} Doing</span>
            <span>&middot;</span><span style={{ color: '#e5c07b' }}>{focusedTasks.length} Focused</span>
          </div>
        </div>

        {/* Task List */}
        <div className="flex-1 overflow-y-auto p-6" style={{ backgroundColor: 'var(--codex-bg)' }}>
          <div className="max-w-4xl mx-auto space-y-3">
            {taskList.length === 0 && !loading && (
              <div className="text-center py-16 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                No tasks yet. Create one to get started.
              </div>
            )}
            {taskList.map((task) => {
              const project = task.projectId ? projectMap.get(task.projectId) : undefined;
              return (
                <div
                  key={task.id} className="p-4 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: 'var(--codex-border)' }}
                  onClick={() => navigate(`/tasks/${task.id}`)}
                  onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = '#1a1a1a'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = '#141414'; }}
                >
                  <div className="flex items-start gap-3">
                    <div className="flex-shrink-0 mt-0.5">{getStatusIcon(task.status)}</div>
                    <div className="flex-1 min-w-0">
                      <div className="mb-2">
                        <div className="text-[14px] mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500, textDecoration: task.status.toLowerCase() === 'archived' ? 'line-through' : 'none' }}>
                          {task.title}
                        </div>
                        {task.description && (
                          <div className="text-[13px] truncate" style={{ color: 'var(--codex-fg-muted)' }}>{task.description}</div>
                        )}
                      </div>

                      {/* Metadata */}
                      <div className="flex items-center gap-3 mb-2 flex-wrap">
                        {task.priority != null && (
                          <div className="flex items-center gap-1.5 text-[12px]">
                            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(task.priority) }} />
                            <span style={{ color: getPriorityColor(task.priority), fontWeight: 500 }}>P{task.priority}</span>
                          </div>
                        )}
                        {task.dueDate && (
                          <div className="flex items-center gap-1.5 text-[12px]" style={{ color: isOverdue(task.dueDate) ? '#e55050' : 'var(--codex-fg-subtle)' }}>
                            <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />{formatDate(task.dueDate)}
                          </div>
                        )}
                        {task.estimatedMinutes != null && (
                          <div className="flex items-center gap-1.5 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                            <Timer className="w-3.5 h-3.5" strokeWidth={1.5} />~{formatTime(task.estimatedMinutes)}
                          </div>
                        )}
                        {project && (
                          <div className="flex items-center gap-1.5 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: project.color }} />
                            {project.name}
                          </div>
                        )}
                      </div>

                      {/* Tags */}
                      {task.tags.length > 0 && (
                        <div className="flex items-center gap-1.5 mb-2 flex-wrap">
                          {task.tags.map(tag => (
                            <span key={tag} className="px-2 py-0.5 rounded text-[11px]" style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)' }}>
                              {tag}
                            </span>
                          ))}
                        </div>
                      )}

                      {/* Indicators */}
                      <div className="flex items-center gap-3 flex-wrap text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                        {task.recurrenceRule && (<div className="flex items-center gap-1"><RefreshCw className="w-3 h-3" strokeWidth={1.5} />Recurring</div>)}
                        {task.focusedAt && (<div className="flex items-center gap-1" style={{ color: '#e5c07b' }}><Zap className="w-3 h-3" strokeWidth={1.5} />Focused</div>)}
                        {task.calendarEventUid && (<div className="flex items-center gap-1"><CalendarIcon className="w-3 h-3" strokeWidth={1.5} />Calendar</div>)}
                        {task.parentId && (<div className="flex items-center gap-1"><Link className="w-3 h-3" strokeWidth={1.5} />Subtask</div>)}
                        {task.totalTrackedSecs > 0 && (<div className="flex items-center gap-1" style={{ color: '#10a37f' }}><Timer className="w-3 h-3" strokeWidth={1.5} />{formatTrackedTime(task.totalTrackedSecs)} tracked</div>)}
                        {task.isTemplate && (<div className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: 'var(--codex-accent-dim)', color: 'var(--codex-accent)', border: '1px solid var(--codex-accent)' }}>TEMPLATE</div>)}
                      </div>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </>
  );
}
