import { useState, useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router';
import {
  Plus,
  Search,
  Filter,
  Clock,
  Calendar as CalendarIcon,
  Zap,
  Link,
  RefreshCw,
  Timer,
  Circle,
  CheckCircle2,
  Archive,
  Loader2,
  AlertTriangle,
  X,
  Trash2,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Task, TaskSummary, Project } from '../../lib/types';
import { RECURRENCE_PRESETS, rruleToLabel } from '../../lib/rrule';

export default function Tasks() {
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterOpen, setFilterOpen] = useState(false);
  const [statusFilters, setStatusFilters] = useState<Set<string>>(
    () => new Set(['todo', 'doing', 'done', 'archived']),
  );
  const [priorityFilters, setPriorityFilters] = useState<Set<number>>(
    () => new Set([0, 1, 2, 3, 4]),
  ); // 0 = no priority

  // Create form state
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newPriority, setNewPriority] = useState('');
  const [newDueDate, setNewDueDate] = useState('');
  const [newTags, setNewTags] = useState('');
  const [newProjectId, setNewProjectId] = useState('');
  const [newEstimatedMinutes, setNewEstimatedMinutes] = useState('');
  const [newIsTemplate, setNewIsTemplate] = useState(false);
  const [newRecurrenceRule, setNewRecurrenceRule] = useState('');

  // View mode: tasks vs templates
  const [viewMode, setViewMode] = useState<'tasks' | 'templates'>('tasks');

  // Delete state
  const [deletingTask, setDeletingTask] = useState<string | null>(null);

  const { data: tasks, loading, error, setData } = useApi<Task[]>('/api/tasks');
  const { data: summary, setData: setSummary } = useApi<TaskSummary>('/api/tasks/summary');
  const { data: projects } = useApi<Project[]>('/api/projects');
  const { data: templates, loading: templatesLoading, setData: setTemplates } = useApi<Task[]>('/api/tasks?templatesOnly=true');

  // Build project lookup map
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

  // Filter tasks client-side
  const filteredTasks = useMemo(() => {
    let list = tasks ?? [];
    // Status filter
    list = list.filter(t => statusFilters.has(t.status.toLowerCase()));
    // Priority filter (0 = tasks with no priority)
    list = list.filter(t => {
      const p = t.priority ?? 0;
      return priorityFilters.has(p);
    });
    // Search
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(t =>
        t.title.toLowerCase().includes(q) ||
        (t.description && t.description.toLowerCase().includes(q)) ||
        t.tags.some(tag => tag.toLowerCase().includes(q)),
      );
    }
    return list;
  }, [tasks, statusFilters, priorityFilters, searchQuery]);

  const taskList = tasks ?? [];
  const activeTasks = taskList.filter(t => { const s = t.status.toLowerCase(); return s === 'todo' || s === 'doing'; });
  const overdueTasks = taskList.filter(t => isOverdue(t.dueDate) && t.status.toLowerCase() !== 'done');
  const completedTasks = taskList.filter(t => t.status.toLowerCase() === 'done');
  const doingTasks = taskList.filter(t => t.status.toLowerCase() === 'doing');
  const focusedTasks = taskList.filter(t => t.focusedAt);

  // Toggle filter helpers
  const toggleStatus = (status: string) => {
    setStatusFilters(prev => {
      const next = new Set(prev);
      if (next.has(status)) next.delete(status);
      else next.add(status);
      return next;
    });
  };

  const togglePriority = (p: number) => {
    setPriorityFilters(prev => {
      const next = new Set(prev);
      if (next.has(p)) next.delete(p);
      else next.add(p);
      return next;
    });
  };

  // Create task
  const handleCreate = useCallback(async () => {
    if (!newTitle.trim()) {
      setCreateError('Title is required');
      return;
    }

    setSaving(true);
    setCreateError(null);

    try {
      const body: Record<string, unknown> = { title: newTitle.trim() };
      if (newDescription.trim()) body.description = newDescription.trim();
      if (newPriority) body.priority = parseInt(newPriority);
      if (newDueDate) body.dueDate = new Date(newDueDate).toISOString();
      if (newTags.trim()) body.tags = newTags.split(',').map(t => t.trim()).filter(Boolean);
      if (newProjectId) body.projectId = newProjectId;
      if (newEstimatedMinutes) body.estimatedMinutes = parseInt(newEstimatedMinutes);
      if (newIsTemplate) {
        body.isTemplate = true;
        if (newRecurrenceRule) body.recurrenceRule = newRecurrenceRule;
      }

      const created = await apiFetch<Task>('/api/tasks', { body });
      setData(prev => [...(prev ?? []), created]);
      setSummary(prev => prev ? { ...prev, todo: prev.todo + 1, total: prev.total + 1 } : prev);
      if (newIsTemplate) {
        setTemplates(prev => [...(prev ?? []), created]);
      }

      // Reset form
      resetCreateForm();
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Failed to create task');
    } finally {
      setSaving(false);
    }
  }, [newTitle, newDescription, newPriority, newDueDate, newTags, newProjectId, newEstimatedMinutes, newIsTemplate, newRecurrenceRule, setData, setSummary, setTemplates]);

  const resetCreateForm = useCallback(() => {
    setCreating(false);
    setCreateError(null);
    setNewTitle('');
    setNewDescription('');
    setNewPriority('');
    setNewDueDate('');
    setNewTags('');
    setNewProjectId('');
    setNewEstimatedMinutes('');
    setNewIsTemplate(false);
    setNewRecurrenceRule('');
  }, []);

  // Delete task
  const confirmDelete = useCallback(async (taskId: string) => {
    const taskBefore = tasks?.find(t => t.id === taskId);
    setData(prev => prev?.filter(t => t.id !== taskId));
    setDeletingTask(null);

    try {
      await apiFetch<void>(`/api/tasks/${encodeURIComponent(taskId)}`, { method: 'DELETE' });
      const freshSummary = await apiFetch<TaskSummary>('/api/tasks/summary');
      setSummary(freshSummary);
    } catch {
      // Revert — re-add the task
      if (taskBefore) {
        setData(prev => [...(prev ?? []), taskBefore]);
      }
    }
  }, [tasks, setData, setSummary]);

  // Quick status cycle: todo → doing → done → todo
  const cycleStatus = useCallback(async (e: React.MouseEvent, task: Task) => {
    e.stopPropagation();
    const order = ['todo', 'doing', 'done'];
    const currentIdx = order.indexOf(task.status.toLowerCase());
    const nextStatus = order[(currentIdx + 1) % order.length];

    setData(prev => prev?.map(t => t.id === task.id ? { ...t, status: nextStatus } : t));

    try {
      await apiFetch(`/api/tasks/${encodeURIComponent(task.id)}`, {
        method: 'PATCH',
        body: { status: nextStatus },
      });
      const freshSummary = await apiFetch<TaskSummary>('/api/tasks/summary');
      setSummary(freshSummary);
    } catch {
      setData(prev => prev?.map(t => t.id === task.id ? { ...t, status: task.status } : t));
    }
  }, [setData, setSummary]);

  const inputStyle = {
    backgroundColor: 'var(--codex-bg)',
    color: 'var(--codex-fg)',
    border: '1px solid var(--codex-border)',
  };

  const focusHandler = (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) =>
    e.currentTarget.style.borderColor = 'var(--codex-accent)';
  const blurHandler = (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) =>
    e.currentTarget.style.borderColor = 'var(--codex-border)';

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
            <button
              onClick={() => setCreating(true)}
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
                onFocus={focusHandler}
                onBlur={blurHandler}
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
                          {['todo', 'doing', 'done', 'archived'].map(status => (
                            <label key={status} className="flex items-center gap-2 text-[13px] cursor-pointer" style={{ color: 'var(--codex-fg-subtle)' }}>
                              <input
                                type="checkbox"
                                checked={statusFilters.has(status)}
                                onChange={() => toggleStatus(status)}
                              />
                              {status.charAt(0).toUpperCase() + status.slice(1)}
                            </label>
                          ))}
                        </div>
                      </div>
                      <div>
                        <div className="text-[12px] mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Priority</div>
                        <div className="space-y-2">
                          {([1, 2, 3, 4] as number[]).map(p => (
                            <label key={p} className="flex items-center gap-2 text-[13px] cursor-pointer" style={{ color: 'var(--codex-fg-subtle)' }}>
                              <input
                                type="checkbox"
                                checked={priorityFilters.has(p)}
                                onChange={() => togglePriority(p)}
                              />
                              <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(p) }} />
                              P{p}
                            </label>
                          ))}
                          <label className="flex items-center gap-2 text-[13px] cursor-pointer" style={{ color: 'var(--codex-fg-subtle)' }}>
                            <input
                              type="checkbox"
                              checked={priorityFilters.has(0)}
                              onChange={() => togglePriority(0)}
                            />
                            No priority
                          </label>
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
            {/* Tab toggle */}
            <div className="flex items-center gap-1 p-1 rounded-lg mb-4" style={{ backgroundColor: 'var(--codex-bg)' }}>
              <button
                onClick={() => setViewMode('tasks')}
                className="px-4 py-1.5 rounded-md text-[12px] transition-colors"
                style={{
                  backgroundColor: viewMode === 'tasks' ? 'var(--codex-bg-tertiary)' : 'transparent',
                  color: viewMode === 'tasks' ? 'var(--codex-fg)' : 'var(--codex-fg-subtle)',
                  fontWeight: viewMode === 'tasks' ? 500 : 400,
                }}
              >
                Tasks
              </button>
              <button
                onClick={() => setViewMode('templates')}
                className="px-4 py-1.5 rounded-md text-[12px] transition-colors"
                style={{
                  backgroundColor: viewMode === 'templates' ? 'var(--codex-bg-tertiary)' : 'transparent',
                  color: viewMode === 'templates' ? 'var(--codex-fg)' : 'var(--codex-fg-subtle)',
                  fontWeight: viewMode === 'templates' ? 500 : 400,
                }}
              >
                Templates
              </button>
            </div>
            {/* Create Form */}
            <AnimatePresence>
              {creating && (
                <motion.div
                  initial={{ opacity: 0, y: -10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  className="rounded-lg border p-6 mb-4"
                  style={{ backgroundColor: '#141414', borderColor: 'var(--codex-accent)' }}
                >
                  <div className="flex items-center justify-between mb-5">
                    <h2 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                      Create Task
                    </h2>
                    <button onClick={resetCreateForm} style={{ color: 'var(--codex-fg-subtle)' }}>
                      <X className="w-4 h-4" strokeWidth={1.5} />
                    </button>
                  </div>

                  <div className="space-y-4">
                    {/* Title */}
                    <div>
                      <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                        Title *
                      </label>
                      <input
                        value={newTitle}
                        onChange={(e) => setNewTitle(e.target.value)}
                        placeholder="Task title..."
                        className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                        style={inputStyle}
                        onFocus={focusHandler}
                        onBlur={blurHandler}
                        onKeyDown={(e) => { if (e.key === 'Enter') handleCreate(); }}
                        autoFocus
                      />
                    </div>

                    {/* Description */}
                    <div>
                      <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                        Description
                      </label>
                      <textarea
                        value={newDescription}
                        onChange={(e) => setNewDescription(e.target.value)}
                        placeholder="Optional description..."
                        rows={2}
                        className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors resize-none"
                        style={inputStyle}
                        onFocus={focusHandler}
                        onBlur={blurHandler}
                      />
                    </div>

                    {/* Row: Priority, Due Date, Estimated */}
                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                          Priority
                        </label>
                        <select
                          value={newPriority}
                          onChange={(e) => setNewPriority(e.target.value)}
                          className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={inputStyle}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        >
                          <option value="">None</option>
                          <option value="1">P1 - Critical</option>
                          <option value="2">P2 - High</option>
                          <option value="3">P3 - Medium</option>
                          <option value="4">P4 - Low</option>
                        </select>
                      </div>
                      <div>
                        <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                          Due Date
                        </label>
                        <input
                          type="date"
                          value={newDueDate}
                          onChange={(e) => setNewDueDate(e.target.value)}
                          className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={{ ...inputStyle, colorScheme: 'dark' }}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        />
                      </div>
                      <div>
                        <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                          Est. Minutes
                        </label>
                        <input
                          type="number"
                          min="1"
                          value={newEstimatedMinutes}
                          onChange={(e) => setNewEstimatedMinutes(e.target.value)}
                          placeholder="60"
                          className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        />
                      </div>
                    </div>

                    {/* Row: Tags, Project */}
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                          Tags (comma-separated)
                        </label>
                        <input
                          value={newTags}
                          onChange={(e) => setNewTags(e.target.value)}
                          placeholder="bug, frontend, urgent"
                          className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={inputStyle}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        />
                      </div>
                      <div>
                        <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                          Project
                        </label>
                        <select
                          value={newProjectId}
                          onChange={(e) => setNewProjectId(e.target.value)}
                          className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={inputStyle}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        >
                          <option value="">None</option>
                          {(projects ?? []).map(p => (
                            <option key={p.id} value={p.id}>{p.name}</option>
                          ))}
                        </select>
                      </div>
                    </div>

                    {/* Template toggle + Recurrence */}
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                          Template
                        </label>
                        <label className="flex items-center gap-2 cursor-pointer py-2">
                          <input
                            type="checkbox"
                            checked={newIsTemplate}
                            onChange={(e) => {
                              setNewIsTemplate(e.target.checked);
                              if (!e.target.checked) setNewRecurrenceRule('');
                            }}
                            className="rounded"
                            style={{ accentColor: 'var(--codex-accent)' }}
                          />
                          <span className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>
                            Is template
                          </span>
                        </label>
                      </div>
                      {newIsTemplate && (
                        <div>
                          <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                            Recurrence
                          </label>
                          <select
                            value={newRecurrenceRule}
                            onChange={(e) => setNewRecurrenceRule(e.target.value)}
                            className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                            style={inputStyle}
                            onFocus={focusHandler}
                            onBlur={blurHandler}
                          >
                            {RECURRENCE_PRESETS.map(p => (
                              <option key={p.value} value={p.value}>{p.label}</option>
                            ))}
                          </select>
                        </div>
                      )}
                    </div>

                    {createError && (
                      <div className="text-[12px] flex items-center gap-1.5" style={{ color: '#ef4444' }}>
                        <AlertTriangle className="w-3.5 h-3.5" strokeWidth={1.5} />
                        {createError}
                      </div>
                    )}

                    <div className="flex justify-end gap-3 pt-2">
                      <button
                        onClick={resetCreateForm}
                        className="px-4 py-2 rounded-lg text-[13px] transition-colors"
                        style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}
                        onMouseEnter={(e) => e.currentTarget.style.borderColor = 'var(--codex-fg-subtle)'}
                        onMouseLeave={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                      >
                        Cancel
                      </button>
                      <button
                        onClick={handleCreate}
                        disabled={saving}
                        className="flex items-center gap-2 px-4 py-2 rounded-lg text-[13px] transition-colors"
                        style={{ backgroundColor: 'var(--codex-accent)', color: 'white', opacity: saving ? 0.7 : 1 }}
                        onMouseEnter={(e) => !saving && (e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)')}
                        onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = 'var(--codex-accent)')}
                      >
                        {saving && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                        Create Task
                      </button>
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Templates view */}
            {viewMode === 'templates' && (
              <div className="space-y-2">
                {templatesLoading ? (
                  <div className="flex items-center justify-center py-16">
                    <Loader2 className="w-5 h-5 animate-spin" style={{ color: 'var(--codex-fg-subtle)' }} />
                  </div>
                ) : (templates ?? []).length === 0 ? (
                  <div className="text-center py-16" style={{ color: 'var(--codex-fg-subtle)' }}>
                    <RefreshCw className="w-10 h-10 mx-auto mb-3" strokeWidth={1} />
                    <p className="text-[14px]">No templates yet</p>
                    <p className="text-[12px] mt-1">Create a template to automate recurring tasks</p>
                  </div>
                ) : (
                  (templates ?? []).map((tmpl, index) => (
                    <motion.button
                      key={tmpl.id}
                      initial={{ opacity: 0, y: 10 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ delay: index * 0.03 }}
                      onClick={() => navigate(`/tasks/${tmpl.id}`)}
                      className="w-full text-left p-4 rounded-lg border transition-colors"
                      style={{ backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)' }}
                      onMouseEnter={(e) => e.currentTarget.style.borderColor = 'var(--codex-accent)'}
                      onMouseLeave={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                    >
                      <div className="flex items-center gap-3 mb-1">
                        <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide" style={{
                          backgroundColor: 'var(--codex-accent-dim)',
                          color: 'var(--codex-accent)',
                          border: '1px solid var(--codex-border)',
                        }}>
                          Template
                        </span>
                        <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)' }}>{tmpl.title}</h3>
                      </div>
                      <div className="flex items-center gap-3 mt-2 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                        {tmpl.recurrenceRule && (
                          <span className="flex items-center gap-1">
                            <RefreshCw className="w-3 h-3" strokeWidth={1.5} />
                            {rruleToLabel(tmpl.recurrenceRule)}
                          </span>
                        )}
                        {tmpl.tags.length > 0 && (
                          <span>{tmpl.tags.join(', ')}</span>
                        )}
                      </div>
                    </motion.button>
                  ))
                )}
              </div>
            )}

            {/* Tasks view */}
            {viewMode === 'tasks' && (
              <>
                {filteredTasks.length === 0 && !loading && !searchQuery && (
                  <div className="text-center py-16 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                    No tasks yet. Create one to get started.
                  </div>
                )}

                {filteredTasks.length === 0 && searchQuery && (
                  <div className="text-center py-16 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                    No tasks matching "{searchQuery}"
                  </div>
                )}

                {filteredTasks.map((task) => {
                  const project = task.projectId ? projectMap.get(task.projectId) : undefined;
                  return (
                    <div key={task.id}>
                      <div
                        className="group p-4 rounded-lg border cursor-pointer transition-all"
                        style={{ backgroundColor: '#141414', borderColor: 'var(--codex-border)' }}
                        onClick={() => navigate(`/tasks/${task.id}`)}
                        onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = '#1a1a1a'; }}
                        onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = '#141414'; }}
                      >
                        <div className="flex items-start gap-3">
                          {/* Clickable status icon to cycle status */}
                          <button
                            className="flex-shrink-0 mt-0.5 hover:scale-110 transition-transform"
                            onClick={(e) => cycleStatus(e, task)}
                            title={`Status: ${task.status} (click to cycle)`}
                          >
                            {getStatusIcon(task.status)}
                          </button>
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

                          {/* Delete button */}
                          <button
                            onClick={(e) => { e.stopPropagation(); setDeletingTask(deletingTask === task.id ? null : task.id); }}
                            title="Delete task"
                            className="flex-shrink-0 p-1.5 rounded transition-colors opacity-0 group-hover:opacity-100"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                            onMouseEnter={(e) => { e.currentTarget.style.color = '#ef4444'; e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; }}
                            onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--codex-fg-subtle)'; e.currentTarget.style.backgroundColor = 'transparent'; }}
                          >
                            <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
                          </button>
                        </div>
                      </div>

                      {/* Delete confirmation */}
                      <AnimatePresence>
                        {deletingTask === task.id && (
                          <motion.div
                            initial={{ opacity: 0, height: 0 }}
                            animate={{ opacity: 1, height: 'auto' }}
                            exit={{ opacity: 0, height: 0 }}
                            className="mt-1 p-3 rounded-lg border"
                            style={{ backgroundColor: 'rgba(239, 68, 68, 0.05)', borderColor: 'rgba(239, 68, 68, 0.2)' }}
                          >
                            <p className="text-[12px] mb-3" style={{ color: 'var(--codex-fg-muted)' }}>
                              Delete <strong style={{ color: 'var(--codex-fg)' }}>{task.title}</strong>? This cannot be undone.
                            </p>
                            <div className="flex items-center gap-2">
                              <button
                                onClick={(e) => { e.stopPropagation(); confirmDelete(task.id); }}
                                className="px-3 py-1.5 rounded text-[12px] transition-colors"
                                style={{ backgroundColor: '#ef4444', color: 'white' }}
                                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#dc2626'}
                                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#ef4444'}
                              >
                                Delete
                              </button>
                              <button
                                onClick={(e) => { e.stopPropagation(); setDeletingTask(null); }}
                                className="px-3 py-1.5 rounded text-[12px] transition-colors"
                                style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}
                              >
                                Cancel
                              </button>
                            </div>
                          </motion.div>
                        )}
                      </AnimatePresence>
                    </div>
                  );
                })}
              </>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
