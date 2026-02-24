import { useState } from 'react';
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
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';

type TaskStatus = 'Todo' | 'Doing' | 'Done' | 'Archived';
type TaskPriority = 1 | 2 | 3 | 4;
type ProjectColor = 'red' | 'orange' | 'yellow' | 'green' | 'blue' | 'purple' | 'gray';

type Task = {
  id: string;
  title: string;
  description?: string;
  status: TaskStatus;
  priority: TaskPriority;
  due_date?: string;
  estimated_minutes?: number;
  project?: string;
  projectColor?: ProjectColor;
  tags: string[];
  recurrence_rule?: string;
  focused_at?: string;
  calendar_event_uid?: string;
  subtasks?: string[];
  blocked_by?: string[];
  attachments?: { id: string; type: string; title: string; value: string }[];
  total_tracked_secs?: number;
  is_template?: boolean;
  created_at: string;
  updated_at: string;
  completed_at?: string;
};

export default function Tasks() {
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterOpen, setFilterOpen] = useState(false);

  const tasks: Task[] = [
    {
      id: '3f2a1b8c', title: 'Fix auth token refresh',
      description: 'Implement automatic token refresh mechanism for expired JWT tokens',
      status: 'Doing', priority: 1, due_date: '2026-02-24', estimated_minutes: 60,
      project: 'Backend', projectColor: 'blue', tags: ['backend', 'security'],
      focused_at: '2026-02-24T14:00:00', calendar_event_uid: 'auth-work-session@calendar',
      blocked_by: ['9d7e4f2a'], total_tracked_secs: 2700,
      created_at: '2026-02-20', updated_at: '2026-02-24',
    },
    {
      id: '7b8c9d0e', title: 'Update API docs',
      description: 'Update authentication endpoints documentation with new token refresh flow',
      status: 'Todo', priority: 3, due_date: '2026-02-25', estimated_minutes: 30,
      project: 'Documentation', projectColor: 'orange', tags: ['docs'],
      attachments: [
        { id: 'a1', type: 'URL', title: 'API Spec Reference', value: 'https://api.example.com/spec' },
        { id: 'a2', type: 'Note', title: 'Notes', value: 'Remember to update authentication section' },
      ],
      created_at: '2026-02-22', updated_at: '2026-02-23',
    },
    {
      id: '9d7e4f2a', title: 'Review PR #142',
      description: 'Code review for authentication refactor pull request',
      status: 'Todo', priority: 2, due_date: '2026-02-22', estimated_minutes: 15,
      tags: ['review'], created_at: '2026-02-21', updated_at: '2026-02-21',
    },
    {
      id: '4e5f6a7b', title: 'Refactor storage layer',
      description: 'Refactor database storage layer to use new ORM patterns',
      status: 'Todo', priority: 2, due_date: '2026-02-26', estimated_minutes: 120,
      project: 'Backend', projectColor: 'blue', tags: ['backend', 'tech-debt'],
      subtasks: ['Migrate models', 'Update queries', 'Add tests'],
      created_at: '2026-02-23', updated_at: '2026-02-23',
    },
    {
      id: '1a2b3c4d', title: 'Daily standup notes',
      status: 'Todo', priority: 4, is_template: true,
      recurrence_rule: 'FREQ=DAILY;BYHOUR=9', tags: ['standup'],
      created_at: '2026-02-01', updated_at: '2026-02-24',
    },
    {
      id: '8c9d0e1f', title: 'Setup CI/CD pipeline',
      description: 'Configure GitHub Actions for automated testing and deployment',
      status: 'Done', priority: 1, completed_at: '2026-02-22',
      tags: ['devops'], total_tracked_secs: 7200,
      created_at: '2026-02-21', updated_at: '2026-02-22',
    },
  ];

  const getPriorityColor = (priority: TaskPriority) => {
    switch (priority) {
      case 1: return '#e55050';
      case 2: return '#d4a017';
      case 3: return '#e5c07b';
      case 4: return '#666';
    }
  };

  const getProjectColorHex = (color?: ProjectColor) => {
    switch (color) {
      case 'blue': return '#4a9eff';
      case 'orange': return '#d4a017';
      case 'green': return '#10a37f';
      case 'red': return '#e55050';
      case 'purple': return '#c678dd';
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

  const isOverdue = (dueDate?: string) => dueDate ? dueDate < '2026-02-24' : false;

  const formatDate = (dateStr: string) =>
    new Date(dateStr).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });

  const getStatusIcon = (status: TaskStatus) => {
    switch (status) {
      case 'Todo': return <Circle className="w-5 h-5" strokeWidth={1.5} style={{ color: '#666' }} />;
      case 'Doing': return <Circle className="w-5 h-5 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      case 'Done': return <CheckCircle2 className="w-5 h-5 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      case 'Archived': return <Archive className="w-5 h-5" strokeWidth={1.5} style={{ color: '#666' }} />;
    }
  };

  const activeTasks = tasks.filter(t => t.status === 'Todo' || t.status === 'Doing');
  const overdueTasks = tasks.filter(t => isOverdue(t.due_date) && t.status !== 'Done');
  const completedTasks = tasks.filter(t => t.status === 'Done');
  const doingTasks = tasks.filter(t => t.status === 'Doing');
  const focusedTasks = tasks.filter(t => t.focused_at);

  return (
    <>
      {/* Center Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <div className="border-b px-6 py-4" style={{ borderColor: 'var(--codex-border-subtle)', backgroundColor: 'var(--codex-bg)' }}>
          <div className="flex items-center justify-between mb-4">
            <h1 className="text-xl" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>Tasks</h1>
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
              <input
                type="text" placeholder="Search tasks..." value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2 rounded-lg border outline-none text-[13px]"
                style={{ backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)', color: 'var(--codex-fg)' }}
              />
            </div>
            <div className="relative">
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
                          {([1, 2, 3, 4] as TaskPriority[]).map(p => (
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

          {/* Summary */}
          <div className="flex items-center gap-2 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
            <span>{activeTasks.length} Active</span><span>&middot;</span>
            <span style={{ color: overdueTasks.length > 0 ? '#e55050' : 'var(--codex-fg-subtle)' }}>{overdueTasks.length} Overdue</span>
            <span>&middot;</span><span>{completedTasks.length} Completed</span>
            <span>&middot;</span><span style={{ color: '#10a37f' }}>{doingTasks.length} Doing</span>
            <span>&middot;</span><span style={{ color: '#e5c07b' }}>{focusedTasks.length} Focused</span>
          </div>
        </div>

        {/* Task List */}
        <div className="flex-1 overflow-y-auto p-6" style={{ backgroundColor: 'var(--codex-bg)' }}>
          <div className="max-w-4xl mx-auto space-y-3">
            {tasks.map((task) => (
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
                      <div className="text-[14px] mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500, textDecoration: task.status === 'Archived' ? 'line-through' : 'none' }}>
                        {task.title}
                      </div>
                      {task.description && (
                        <div className="text-[13px] truncate" style={{ color: 'var(--codex-fg-muted)' }}>{task.description}</div>
                      )}
                    </div>

                    {/* Metadata */}
                    <div className="flex items-center gap-3 mb-2 flex-wrap">
                      <div className="flex items-center gap-1.5 text-[12px]">
                        <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(task.priority) }} />
                        <span style={{ color: getPriorityColor(task.priority), fontWeight: 500 }}>P{task.priority}</span>
                      </div>
                      {task.due_date && (
                        <div className="flex items-center gap-1.5 text-[12px]" style={{ color: isOverdue(task.due_date) ? '#e55050' : 'var(--codex-fg-subtle)' }}>
                          <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />{formatDate(task.due_date)}
                        </div>
                      )}
                      {task.estimated_minutes && (
                        <div className="flex items-center gap-1.5 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                          <Timer className="w-3.5 h-3.5" strokeWidth={1.5} />~{formatTime(task.estimated_minutes)}
                        </div>
                      )}
                      {task.project && (
                        <div className="flex items-center gap-1.5 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getProjectColorHex(task.projectColor) }} />
                          {task.project}
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
                      {task.recurrence_rule && (<div className="flex items-center gap-1"><RefreshCw className="w-3 h-3" strokeWidth={1.5} />Recurring</div>)}
                      {task.focused_at && (<div className="flex items-center gap-1" style={{ color: '#e5c07b' }}><Zap className="w-3 h-3" strokeWidth={1.5} />Focused</div>)}
                      {task.calendar_event_uid && (<div className="flex items-center gap-1"><CalendarIcon className="w-3 h-3" strokeWidth={1.5} />Calendar</div>)}
                      {task.subtasks && task.subtasks.length > 0 && (<div className="flex items-center gap-1">{task.subtasks.length} subtask{task.subtasks.length !== 1 ? 's' : ''}</div>)}
                      {task.blocked_by && task.blocked_by.length > 0 && (<div className="flex items-center gap-1" style={{ color: '#e55050' }}><Link className="w-3 h-3" strokeWidth={1.5} />Blocked by {task.blocked_by.length}</div>)}
                      {task.attachments && task.attachments.length > 0 && (<div className="flex items-center gap-1"><Paperclip className="w-3 h-3" strokeWidth={1.5} />{task.attachments.length} file{task.attachments.length !== 1 ? 's' : ''}</div>)}
                      {task.total_tracked_secs && task.total_tracked_secs > 0 && (<div className="flex items-center gap-1" style={{ color: '#10a37f' }}><Timer className="w-3 h-3" strokeWidth={1.5} />{formatTrackedTime(task.total_tracked_secs)} tracked</div>)}
                      {task.is_template && (<div className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: 'var(--codex-accent-dim)', color: 'var(--codex-accent)', border: '1px solid var(--codex-accent)' }}>TEMPLATE</div>)}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}
