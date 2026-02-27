import { useNavigate, useParams } from 'react-router';
import {
  ArrowLeft,
  Calendar as CalendarIcon,
  CalendarPlus,
  Plus,
  Play,
  Square,
  Zap,
  Circle,
  FileText,
  Link,
  StickyNote,
  Loader2,
  AlertTriangle,
  Trash2,
  X,
  Search,
  GitBranch,
  RefreshCw,
} from 'lucide-react';
import { useState, useMemo, useCallback, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Task, TaskAttachment, TaskTimeEntry, Project, TaskDependencies } from '../../lib/types';
import { RECURRENCE_PRESETS, rruleToLabel } from '../../lib/rrule';

export default function TaskDetail() {
  const { id } = useParams();
  const navigate = useNavigate();

  // Timer state (client-side tracking)
  const [timerStartedAt, setTimerStartedAt] = useState<number | null>(null);
  const [timerElapsed, setTimerElapsed] = useState(0);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Editing state
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleValue, setTitleValue] = useState('');
  const [descValue, setDescValue] = useState('');
  const [patchingField, setPatchingField] = useState<string | null>(null);

  // Tag input state
  const [addingTag, setAddingTag] = useState(false);
  const [newTag, setNewTag] = useState('');

  // Delete state
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  // Dependency state
  const [depSearchOpen, setDepSearchOpen] = useState(false);
  const [depSearchQuery, setDepSearchQuery] = useState('');
  const [depSearchResults, setDepSearchResults] = useState<Task[]>([]);
  const [depSearchLoading, setDepSearchLoading] = useState(false);
  const depSearchRef = useRef<HTMLDivElement>(null);

  // Schedule to calendar state
  const [scheduling, setScheduling] = useState(false);
  const [scheduleForm, setScheduleForm] = useState({ start: '', end: '' });
  const [scheduleSaving, setScheduleSaving] = useState(false);

  // Recurrence editing state
  const [editingRecurrence, setEditingRecurrence] = useState(false);
  const [recurrenceValue, setRecurrenceValue] = useState('');

  const { data: task, loading, error, setData: setTask, refetch: refetchTask } = useApi<Task>(`/api/tasks/${id}`);
  const { data: subtasks } = useApi<Task[]>(`/api/tasks/${id}/subtasks`);
  const { data: attachments } = useApi<TaskAttachment[]>(`/api/tasks/${id}/attachments`);
  const { data: timeEntries, refetch: refetchTimeEntries } = useApi<TaskTimeEntry[]>(`/api/tasks/${id}/time-entries`);
  const { data: projects } = useApi<Project[]>('/api/projects');
  const deps = useApi<TaskDependencies>(`/api/tasks/${id}/dependencies`);

  // Sync local state when task loads
  useEffect(() => {
    if (task) {
      setTitleValue(task.title);
      setDescValue(task.description ?? '');
      setRecurrenceValue(task.recurrenceRule ?? '');
    }
  }, [task]);

  // Timer tick
  useEffect(() => {
    if (timerStartedAt) {
      timerRef.current = setInterval(() => {
        setTimerElapsed(Math.floor((Date.now() - timerStartedAt) / 1000));
      }, 1000);
    } else {
      if (timerRef.current) clearInterval(timerRef.current);
      setTimerElapsed(0);
    }
    return () => { if (timerRef.current) clearInterval(timerRef.current); };
  }, [timerStartedAt]);

  // Build project lookup
  const projectMap = useMemo(() => {
    const map = new Map<string, Project>();
    if (projects) for (const p of projects) map.set(p.id, p);
    return map;
  }, [projects]);

  const project = task?.projectId ? projectMap.get(task.projectId) : undefined;

  const getPriorityColor = (priority: number | null) => {
    switch (priority) {
      case 1: return '#e55050';
      case 2: return '#d4a017';
      case 3: return '#e5c07b';
      case 4: return '#666';
      default: return '#666';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status.toLowerCase()) {
      case 'done':
        return <Circle className="w-4 h-4 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      case 'doing':
        return <Circle className="w-4 h-4 fill-current" strokeWidth={1.5} style={{ color: '#10a37f' }} />;
      default:
        return <Circle className="w-4 h-4" strokeWidth={1.5} style={{ color: '#666' }} />;
    }
  };

  const getAttachmentIcon = (attachmentType: string) => {
    switch (attachmentType) {
      case 'File': return <FileText className="w-4 h-4" strokeWidth={1.5} />;
      case 'URL': return <Link className="w-4 h-4" strokeWidth={1.5} />;
      case 'Note': return <StickyNote className="w-4 h-4" strokeWidth={1.5} />;
      default: return <FileText className="w-4 h-4" strokeWidth={1.5} />;
    }
  };

  const formatDate = (dateStr: string) =>
    new Date(dateStr).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });

  const formatDateTime = (isoStr: string) => {
    const date = new Date(isoStr);
    const datePart = date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    const timePart = date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
    return `${datePart} ${timePart}`;
  };

  const formatDuration = (secs: number) => {
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    const remainMins = mins % 60;
    return remainMins > 0 ? `${hours}h ${remainMins}m` : `${hours}h`;
  };

  const formatTimerDisplay = (secs: number) => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    return `${m}:${String(s).padStart(2, '0')}`;
  };

  // ── Patch helper ──────────────────────────────────────────────────────────────
  const patchTask = useCallback(async (field: string, body: Record<string, unknown>) => {
    if (!id) return;
    setPatchingField(field);
    try {
      const updated = await apiFetch<Task>(`/api/tasks/${encodeURIComponent(id)}`, {
        method: 'PATCH',
        body,
      });
      setTask(updated);
    } catch {
      // Revert by refetching
      refetchTask();
    } finally {
      setPatchingField(null);
    }
  }, [id, setTask, refetchTask]);

  // ── Status change ─────────────────────────────────────────────────────────────
  const handleStatusChange = useCallback(async (newStatus: string) => {
    if (!task || newStatus === task.status) return;
    // Optimistic
    setTask(prev => prev ? { ...prev, status: newStatus } : prev);
    await patchTask('status', { status: newStatus });
  }, [task, setTask, patchTask]);

  // ── Title edit ────────────────────────────────────────────────────────────────
  const handleTitleBlur = useCallback(async () => {
    setEditingTitle(false);
    if (!task || titleValue.trim() === task.title) return;
    if (!titleValue.trim()) {
      setTitleValue(task.title); // revert empty
      return;
    }
    setTask(prev => prev ? { ...prev, title: titleValue.trim() } : prev);
    await patchTask('title', { title: titleValue.trim() });
  }, [task, titleValue, setTask, patchTask]);

  // ── Description edit ──────────────────────────────────────────────────────────
  const handleDescBlur = useCallback(async () => {
    if (!task) return;
    const newDesc = descValue.trim() || null;
    if (newDesc === (task.description ?? null)) return;
    setTask(prev => prev ? { ...prev, description: newDesc } : prev);
    await patchTask('description', { description: newDesc });
  }, [task, descValue, setTask, patchTask]);

  // ── Priority change ───────────────────────────────────────────────────────────
  const handlePriorityChange = useCallback(async (value: string) => {
    if (!task) return;
    const newPriority = value ? parseInt(value) : null;
    if (newPriority === task.priority) return;
    setTask(prev => prev ? { ...prev, priority: newPriority } : prev);
    await patchTask('priority', { priority: newPriority });
  }, [task, setTask, patchTask]);

  // ── Due date change ───────────────────────────────────────────────────────────
  const handleDueDateChange = useCallback(async (value: string) => {
    if (!task) return;
    const newDate = value ? new Date(value).toISOString() : null;
    setTask(prev => prev ? { ...prev, dueDate: newDate } : prev);
    await patchTask('dueDate', { dueDate: newDate });
  }, [task, setTask, patchTask]);

  // ── Estimated minutes change ──────────────────────────────────────────────────
  const handleEstimatedChange = useCallback(async (value: string) => {
    if (!task) return;
    const newEst = value ? parseInt(value) : null;
    if (newEst === task.estimatedMinutes) return;
    setTask(prev => prev ? { ...prev, estimatedMinutes: newEst } : prev);
    await patchTask('estimatedMinutes', { estimatedMinutes: newEst });
  }, [task, setTask, patchTask]);

  // ── Project reassignment ─────────────────────────────────────────────────────
  const handleProjectChange = useCallback(async (value: string) => {
    if (!task) return;
    const newProjectId = value || null;
    if (newProjectId === (task.projectId ?? null)) return;
    setTask(prev => prev ? { ...prev, projectId: newProjectId } : prev);
    await patchTask('projectId', { projectId: newProjectId });
  }, [task, setTask, patchTask]);

  // ── Tag management ────────────────────────────────────────────────────────────
  const handleAddTag = useCallback(async () => {
    if (!task || !newTag.trim()) return;
    const tag = newTag.trim().toLowerCase();
    if (task.tags.includes(tag)) { setNewTag(''); setAddingTag(false); return; }
    const newTags = [...task.tags, tag];
    setTask(prev => prev ? { ...prev, tags: newTags } : prev);
    setNewTag('');
    setAddingTag(false);
    await patchTask('tags', { tags: newTags });
  }, [task, newTag, setTask, patchTask]);

  const handleRemoveTag = useCallback(async (tag: string) => {
    if (!task) return;
    const newTags = task.tags.filter(t => t !== tag);
    setTask(prev => prev ? { ...prev, tags: newTags } : prev);
    await patchTask('tags', { tags: newTags });
  }, [task, setTask, patchTask]);

  // ── Timer start/stop ──────────────────────────────────────────────────────────
  const handleTimerStart = useCallback(() => {
    setTimerStartedAt(Date.now());
  }, []);

  const handleTimerStop = useCallback(async () => {
    if (!timerStartedAt || !id) return;
    const durationSecs = Math.floor((Date.now() - timerStartedAt) / 1000);
    setTimerStartedAt(null);

    if (durationSecs < 1) return; // skip trivial entries

    try {
      await apiFetch(`/api/tasks/${encodeURIComponent(id)}/time-entries`, {
        body: {
          source: 'manual',
          startedAt: new Date(timerStartedAt).toISOString(),
          durationSecs,
        },
      });
      refetchTimeEntries();
      refetchTask(); // updates totalTrackedSecs
    } catch {
      // Silently fail — time entry may still have been created
    }
  }, [timerStartedAt, id, refetchTimeEntries, refetchTask]);

  // ── Focus / Unfocus ───────────────────────────────────────────────────────────
  const handleFocus = useCallback(async () => {
    if (!id) return;
    try {
      await apiFetch(`/api/tasks/${encodeURIComponent(id)}/focus`, {
        method: 'POST',
      });
      refetchTask();
    } catch {
      // May fail if max focus slots reached
    }
  }, [id, refetchTask]);

  const handleUnfocus = useCallback(async () => {
    if (!id) return;
    try {
      await apiFetch(`/api/tasks/${encodeURIComponent(id)}/focus`, {
        method: 'DELETE',
      });
      refetchTask();
    } catch {
      // Silently fail
    }
  }, [id, refetchTask]);

  // ── Delete task ───────────────────────────────────────────────────────────────
  const handleDelete = useCallback(async () => {
    if (!id) return;
    try {
      await apiFetch<void>(`/api/tasks/${encodeURIComponent(id)}`, { method: 'DELETE' });
      navigate('/tasks');
    } catch {
      setConfirmingDelete(false);
    }
  }, [id, navigate]);

  // ── Recurrence save ─────────────────────────────────────────────────────────
  const handleSaveRecurrence = useCallback(async () => {
    if (!task) return;
    setPatchingField('recurrenceRule');
    try {
      const updated = await apiFetch<Task>(`/api/tasks/${encodeURIComponent(id!)}`, {
        method: 'PATCH',
        body: { recurrenceRule: recurrenceValue },
      });
      setTask(updated);
      setEditingRecurrence(false);
    } catch { /* ignore */ }
    finally { setPatchingField(null); }
  }, [task, id, recurrenceValue, setTask]);

  // ── Dependency search ─────────────────────────────────────────────────────
  useEffect(() => {
    if (!depSearchOpen || !depSearchQuery.trim()) {
      setDepSearchResults([]);
      return;
    }
    let cancelled = false;
    setDepSearchLoading(true);
    apiFetch<Task[]>('/api/tasks', { params: { limit: 20 } })
      .then((tasks) => {
        if (cancelled) return;
        const q = depSearchQuery.toLowerCase();
        const blockedByIds = new Set((deps.data?.blockedBy ?? []).map(t => t.id));
        const blocksIds = new Set((deps.data?.blocks ?? []).map(t => t.id));
        const filtered = tasks.filter(t =>
          t.id !== id &&
          !blockedByIds.has(t.id) &&
          !blocksIds.has(t.id) &&
          t.title.toLowerCase().includes(q)
        );
        setDepSearchResults(filtered.slice(0, 8));
      })
      .catch(() => { if (!cancelled) setDepSearchResults([]); })
      .finally(() => { if (!cancelled) setDepSearchLoading(false); });
    return () => { cancelled = true; };
  }, [depSearchQuery, depSearchOpen, id, deps.data]);

  // Close dep search on outside click
  useEffect(() => {
    if (!depSearchOpen) return;
    const handler = (e: MouseEvent) => {
      if (depSearchRef.current && !depSearchRef.current.contains(e.target as Node)) {
        setDepSearchOpen(false);
        setDepSearchQuery('');
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [depSearchOpen]);

  const handleAddDependency = useCallback(async (blockerId: string) => {
    if (!id) return;
    try {
      await apiFetch(`/api/tasks/${encodeURIComponent(id)}/dependencies`, {
        body: { blockerId },
      });
      deps.refetch();
    } catch {
      // Silently fail
    }
    setDepSearchOpen(false);
    setDepSearchQuery('');
  }, [id, deps]);

  const handleRemoveBlockedBy = useCallback(async (blockerId: string) => {
    if (!id) return;
    try {
      await apiFetch<void>(`/api/tasks/${encodeURIComponent(id)}/dependencies/${encodeURIComponent(blockerId)}`, {
        method: 'DELETE',
      });
      deps.refetch();
    } catch {
      // Silently fail
    }
  }, [id, deps]);

  const handleRemoveBlocks = useCallback(async (blockedId: string) => {
    if (!id) return;
    try {
      await apiFetch<void>(`/api/tasks/${encodeURIComponent(blockedId)}/dependencies/${encodeURIComponent(id)}`, {
        method: 'DELETE',
      });
      deps.refetch();
    } catch {
      // Silently fail
    }
  }, [id, deps]);

  // ── Schedule to calendar ─────────────────────────────────────────────────────
  async function handleScheduleToCalendar() {
    if (!task || !scheduleForm.start || !scheduleForm.end) return;
    setScheduleSaving(true);
    try {
      await apiFetch('/api/calendar/events', {
        body: {
          summary: task.title,
          description: task.description || undefined,
          start: new Date(scheduleForm.start).toISOString(),
          end: new Date(scheduleForm.end).toISOString(),
        },
      });
      setScheduling(false);
      setScheduleForm({ start: '', end: '' });
    } catch { /* ignore */ } finally {
      setScheduleSaving(false);
    }
  }

  // ── DAG helpers ──────────────────────────────────────────────────────────────
  const getStatusDotColor = (status: string) => {
    switch (status.toLowerCase()) {
      case 'done': return '#10a37f';
      case 'doing': return '#e5c07b';
      default: return '#666';
    }
  };

  const truncateTitle = (title: string, max: number) =>
    title.length > max ? title.slice(0, max) + '...' : title;

  if (loading && !task) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <Loader2 className="w-5 h-5 animate-spin" style={{ color: 'var(--codex-fg-subtle)' }} />
      </div>
    );
  }

  if (error && !task) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2" style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg-subtle)' }}>
        <AlertTriangle className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px]">Failed to load task</span>
      </div>
    );
  }

  if (!task) return null;

  const subtaskList = subtasks ?? [];
  const attachmentList = attachments ?? [];
  const timeEntryList = timeEntries ?? [];
  const isTimerRunning = timerStartedAt !== null;
  const currentStatus = task.status.toLowerCase();

  // For date input: convert ISO string to YYYY-MM-DD
  const dueDateInputValue = task.dueDate ? task.dueDate.split('T')[0] : '';

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header Bar */}
        <div className="border-b px-6 py-4" style={{
          borderColor: 'var(--codex-border-subtle)',
          backgroundColor: 'var(--codex-bg)'
        }}>
          <div className="flex items-center gap-4">
            <button
              onClick={() => navigate('/tasks')}
              className="flex items-center gap-2 text-[13px] transition-colors"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg)'}
              onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
            >
              <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
              Tasks
            </button>

            <div className="text-[13px]" style={{
              color: '#10a37f',
              fontFamily: 'var(--font-mono)'
            }}>
              {task.id.slice(0, 8)}
            </div>

            {/* Status select */}
            <select
              value={currentStatus.charAt(0).toUpperCase() + currentStatus.slice(1)}
              onChange={(e) => handleStatusChange(e.target.value.toLowerCase())}
              disabled={patchingField === 'status'}
              className="px-3 py-1.5 rounded text-[12px] outline-none cursor-pointer"
              style={{
                backgroundColor: currentStatus === 'doing' ? 'rgba(16, 163, 127, 0.2)' : '#141414',
                color: currentStatus === 'doing' ? '#10a37f' : 'var(--codex-fg-subtle)',
                border: `1px solid ${currentStatus === 'doing' ? '#10a37f' : 'var(--codex-border)'}`
              }}
            >
              <option>Todo</option>
              <option>Doing</option>
              <option>Done</option>
              <option>Archived</option>
            </select>

            {/* Priority badge */}
            {task.priority != null && (
              <div className="flex items-center gap-2 px-3 py-1.5 rounded text-[12px]" style={{
                backgroundColor: getPriorityColor(task.priority) + '20',
                color: getPriorityColor(task.priority),
                border: `1px solid ${getPriorityColor(task.priority)}40`
              }}>
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(task.priority) }} />
                P{task.priority}
              </div>
            )}

            {/* Focus button (if not focused) */}
            {!task.focusedAt && (
              <button
                onClick={handleFocus}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] transition-colors"
                style={{ color: '#e5c07b', border: '1px solid #e5c07b40' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'rgba(229, 192, 123, 0.1)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
              >
                <Zap className="w-3.5 h-3.5" strokeWidth={1.5} />
                Focus
              </button>
            )}

            {/* Schedule to calendar button */}
            <button
              onClick={() => {
                if (task.dueDate) {
                  const due = new Date(task.dueDate);
                  const start = due.toISOString().slice(0, 16);
                  const end = task.estimatedMinutes
                    ? new Date(due.getTime() + task.estimatedMinutes * 60000).toISOString().slice(0, 16)
                    : new Date(due.getTime() + 3600000).toISOString().slice(0, 16);
                  setScheduleForm({ start, end });
                }
                setScheduling(!scheduling);
              }}
              title="Schedule to calendar"
              className="p-2 rounded transition-colors"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={e => { e.currentTarget.style.color = 'var(--codex-accent)'; e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; }}
              onMouseLeave={e => { e.currentTarget.style.color = 'var(--codex-fg-subtle)'; e.currentTarget.style.backgroundColor = 'transparent'; }}
            >
              <CalendarPlus className="w-4 h-4" strokeWidth={1.5} />
            </button>

            <div className="flex-1" />

            {/* Delete button */}
            <button
              onClick={() => setConfirmingDelete(!confirmingDelete)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] transition-colors"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) => { e.currentTarget.style.color = '#ef4444'; e.currentTarget.style.backgroundColor = 'rgba(239, 68, 68, 0.1)'; }}
              onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--codex-fg-subtle)'; e.currentTarget.style.backgroundColor = 'transparent'; }}
            >
              <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
              Delete
            </button>
          </div>

          {/* Delete confirmation inline */}
          <AnimatePresence>
            {confirmingDelete && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: 'auto' }}
                exit={{ opacity: 0, height: 0 }}
                className="mt-3 p-3 rounded-lg border"
                style={{ backgroundColor: 'rgba(239, 68, 68, 0.05)', borderColor: 'rgba(239, 68, 68, 0.2)' }}
              >
                <p className="text-[12px] mb-3" style={{ color: 'var(--codex-fg-muted)' }}>
                  Delete <strong style={{ color: 'var(--codex-fg)' }}>{task.title}</strong>? This cannot be undone.
                </p>
                <div className="flex items-center gap-2">
                  <button
                    onClick={handleDelete}
                    className="px-3 py-1.5 rounded text-[12px] transition-colors"
                    style={{ backgroundColor: '#ef4444', color: 'white' }}
                    onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#dc2626'}
                    onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#ef4444'}
                  >
                    Delete
                  </button>
                  <button
                    onClick={() => setConfirmingDelete(false)}
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

        {/* Schedule to Calendar form */}
        <AnimatePresence>
          {scheduling && (
            <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}
              className="border-b px-6 py-4"
              style={{ backgroundColor: '#141414', borderColor: 'var(--codex-border)' }}>
              <div className="space-y-3">
                <h3 className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                  Schedule to Calendar
                </h3>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Start</label>
                    <input type="datetime-local" value={scheduleForm.start}
                      onChange={e => setScheduleForm(f => ({ ...f, start: e.target.value }))}
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none"
                      style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)', colorScheme: 'dark' }} />
                  </div>
                  <div>
                    <label className="block text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>End</label>
                    <input type="datetime-local" value={scheduleForm.end}
                      onChange={e => setScheduleForm(f => ({ ...f, end: e.target.value }))}
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none"
                      style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)', colorScheme: 'dark' }} />
                  </div>
                </div>
                <div className="flex justify-end gap-3">
                  <button onClick={() => setScheduling(false)}
                    className="px-3 py-1.5 rounded text-[12px]"
                    style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}>
                    Cancel
                  </button>
                  <button onClick={handleScheduleToCalendar} disabled={scheduleSaving || !scheduleForm.start || !scheduleForm.end}
                    className="px-3 py-1.5 rounded text-[12px] flex items-center gap-1.5"
                    style={{ backgroundColor: 'var(--codex-accent)', color: 'white', opacity: (scheduleSaving || !scheduleForm.start || !scheduleForm.end) ? 0.5 : 1 }}>
                    {scheduleSaving && <Loader2 className="w-3 h-3 animate-spin" />}
                    Schedule
                  </button>
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Content Area */}
        <div className="flex-1 overflow-y-auto p-8" style={{ backgroundColor: 'var(--codex-bg)' }}>
          <div className="max-w-4xl mx-auto">
            {/* Title — editable input */}
            <input
              type="text"
              value={titleValue}
              onChange={(e) => setTitleValue(e.target.value)}
              onBlur={handleTitleBlur}
              onKeyDown={(e) => { if (e.key === 'Enter') e.currentTarget.blur(); }}
              className="w-full text-2xl font-bold mb-6 bg-transparent border-none outline-none"
              style={{ color: 'var(--codex-fg)' }}
              placeholder="Task title..."
            />

            {/* Description */}
            <div className="mb-8">
              <div className="rounded-lg border" style={{
                backgroundColor: '#141414',
                borderColor: '#1a1a1a'
              }}>
                <textarea
                  value={descValue}
                  onChange={(e) => setDescValue(e.target.value)}
                  onBlur={handleDescBlur}
                  placeholder="Add a description..."
                  className="w-full p-4 min-h-[400px] bg-transparent outline-none resize-none"
                  style={{
                    color: 'var(--codex-fg)',
                    fontSize: '14px',
                    lineHeight: '1.6'
                  }}
                />
              </div>
            </div>

            {/* Attachments */}
            {attachmentList.length > 0 && (
              <div className="mb-8">
                <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                  Attachments
                </h3>
                <div className="grid grid-cols-2 gap-3">
                  {attachmentList.map(att => (
                    <div
                      key={att.id}
                      className="p-3 rounded-lg border cursor-pointer transition-colors"
                      style={{ backgroundColor: '#141414', borderColor: 'var(--codex-border)' }}
                      onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
                      onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#141414'}
                    >
                      <div className="flex items-start gap-3">
                        <div style={{ color: 'var(--codex-fg-subtle)' }}>
                          {getAttachmentIcon(att.attachmentType)}
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)' }}>
                            {att.title ?? att.attachmentType}
                          </div>
                          <div className="text-[11px] truncate" style={{
                            color: 'var(--codex-fg-subtle)',
                            fontFamily: 'var(--font-mono)'
                          }}>
                            {att.value}
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Subtasks */}
            {subtaskList.length > 0 && (
              <div className="mb-8">
                <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                  Subtasks
                </h3>
                <div className="space-y-2">
                  {subtaskList.map(subtask => (
                    <div
                      key={subtask.id}
                      className="flex items-center gap-3 p-3 rounded-lg border cursor-pointer"
                      style={{ backgroundColor: '#141414', borderColor: 'var(--codex-border)' }}
                      onClick={() => navigate(`/tasks/${subtask.id}`)}
                    >
                      {getStatusIcon(subtask.status)}
                      <div className="flex-1 text-[13px]" style={{
                        color: 'var(--codex-fg)',
                        textDecoration: subtask.status.toLowerCase() === 'done' ? 'line-through' : 'none'
                      }}>
                        {subtask.title}
                      </div>
                      {subtask.priority != null && (
                        <div className="flex items-center gap-1.5 text-[11px]">
                          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getPriorityColor(subtask.priority) }} />
                          <span style={{ color: getPriorityColor(subtask.priority) }}>P{subtask.priority}</span>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Right Panel */}
      <aside className="w-[420px] border-l overflow-y-auto" style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)'
      }}>
        <div className="p-6 space-y-6">
          {/* Properties Card */}
          <div className="p-4 rounded-lg" style={{
            backgroundColor: '#141414',
            border: '1px solid var(--codex-border)'
          }}>
            <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
              Properties
            </h3>
            <div className="space-y-3">
              {/* Status */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Status</span>
                <select
                  value={currentStatus.charAt(0).toUpperCase() + currentStatus.slice(1)}
                  onChange={(e) => handleStatusChange(e.target.value.toLowerCase())}
                  disabled={patchingField === 'status'}
                  className="px-2 py-1 rounded text-[11px] outline-none cursor-pointer"
                  style={{
                    backgroundColor: 'var(--codex-bg)',
                    color: 'var(--codex-fg)',
                    border: '1px solid var(--codex-border)'
                  }}
                >
                  <option>Todo</option>
                  <option>Doing</option>
                  <option>Done</option>
                  <option>Archived</option>
                </select>
              </div>

              {/* Priority */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Priority</span>
                <select
                  value={task.priority ?? ''}
                  onChange={(e) => handlePriorityChange(e.target.value)}
                  disabled={patchingField === 'priority'}
                  className="px-2 py-1 rounded text-[11px] outline-none cursor-pointer"
                  style={{
                    backgroundColor: 'var(--codex-bg)',
                    color: task.priority ? getPriorityColor(task.priority) : 'var(--codex-fg-subtle)',
                    border: '1px solid var(--codex-border)'
                  }}
                >
                  <option value="">None</option>
                  <option value="1">P1 - Critical</option>
                  <option value="2">P2 - High</option>
                  <option value="3">P3 - Medium</option>
                  <option value="4">P4 - Low</option>
                </select>
              </div>

              {/* Due Date */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Due Date</span>
                <input
                  type="date"
                  value={dueDateInputValue}
                  onChange={(e) => handleDueDateChange(e.target.value)}
                  disabled={patchingField === 'dueDate'}
                  className="px-2 py-1 rounded text-[11px] outline-none cursor-pointer"
                  style={{
                    backgroundColor: 'var(--codex-bg)',
                    color: 'var(--codex-fg)',
                    border: '1px solid var(--codex-border)',
                    colorScheme: 'dark',
                  }}
                />
              </div>

              {/* Estimated */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Estimated</span>
                <div className="flex items-center gap-1">
                  <input
                    type="number"
                    min="0"
                    value={task.estimatedMinutes ?? ''}
                    onChange={(e) => handleEstimatedChange(e.target.value)}
                    disabled={patchingField === 'estimatedMinutes'}
                    placeholder="—"
                    className="w-16 px-2 py-1 rounded text-[11px] outline-none text-right"
                    style={{
                      backgroundColor: 'var(--codex-bg)',
                      color: 'var(--codex-fg)',
                      border: '1px solid var(--codex-border)',
                    }}
                  />
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>min</span>
                </div>
              </div>

              {/* Project */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Project</span>
                <select
                  value={task.projectId ?? ''}
                  onChange={(e) => handleProjectChange(e.target.value)}
                  disabled={patchingField === 'projectId'}
                  className="px-2 py-1 rounded text-[11px] outline-none cursor-pointer"
                  style={{
                    backgroundColor: 'var(--codex-bg)',
                    color: 'var(--codex-fg)',
                    border: '1px solid var(--codex-border)',
                  }}
                >
                  <option value="">None</option>
                  {(projects ?? []).map(p => (
                    <option key={p.id} value={p.id}>{p.name}</option>
                  ))}
                </select>
              </div>

              {/* Tags */}
              <div>
                <div className="text-[12px] mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>Tags</div>
                <div className="flex flex-wrap gap-1.5">
                  {task.tags.map(tag => (
                    <span
                      key={tag}
                      className="group px-2 py-0.5 rounded text-[11px] cursor-pointer transition-colors"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg-subtle)',
                        border: '1px solid var(--codex-border)'
                      }}
                      onClick={() => handleRemoveTag(tag)}
                      title="Click to remove"
                    >
                      {tag}
                      <X className="w-2.5 h-2.5 inline ml-1 opacity-50 group-hover:opacity-100" strokeWidth={2} />
                    </span>
                  ))}
                  {addingTag ? (
                    <input
                      value={newTag}
                      onChange={(e) => setNewTag(e.target.value)}
                      onBlur={() => { if (newTag.trim()) handleAddTag(); else setAddingTag(false); }}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') handleAddTag();
                        if (e.key === 'Escape') { setAddingTag(false); setNewTag(''); }
                      }}
                      autoFocus
                      placeholder="tag..."
                      className="px-2 py-0.5 rounded text-[11px] outline-none w-20"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg)',
                        border: '1px solid var(--codex-accent)'
                      }}
                    />
                  ) : (
                    <button
                      onClick={() => setAddingTag(true)}
                      className="px-2 py-0.5 rounded text-[11px] transition-colors"
                      style={{
                        backgroundColor: 'transparent',
                        color: 'var(--codex-fg-subtle)',
                        border: '1px solid var(--codex-border)'
                      }}
                      onMouseEnter={(e) => e.currentTarget.style.color = '#10a37f'}
                      onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                    >
                      <Plus className="w-3 h-3" strokeWidth={1.5} />
                    </button>
                  )}
                </div>
              </div>

              <div className="flex justify-between text-[12px] pt-2 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Created</span>
                <span style={{ color: 'var(--codex-fg)' }}>{formatDate(task.createdAt)}</span>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Updated</span>
                <span style={{ color: 'var(--codex-fg)' }}>{formatDate(task.updatedAt)}</span>
              </div>
            </div>
          </div>

          {/* Recurrence card (templates only) */}
          {task.isTemplate && (
            <div className="rounded-lg border p-5" style={{ backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)' }}>
              <div className="flex items-center justify-between mb-4">
                <h3 className="flex items-center gap-2 text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                  <RefreshCw className="w-4 h-4" strokeWidth={1.5} />
                  Recurrence
                </h3>
                {!editingRecurrence && (
                  <button
                    onClick={() => setEditingRecurrence(true)}
                    className="text-[11px] transition-colors"
                    style={{ color: 'var(--codex-accent)' }}
                    onMouseEnter={(e) => e.currentTarget.style.opacity = '0.8'}
                    onMouseLeave={(e) => e.currentTarget.style.opacity = '1'}
                  >
                    Edit
                  </button>
                )}
              </div>

              {editingRecurrence ? (
                <div className="space-y-3">
                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                      Frequency
                    </label>
                    <select
                      value={recurrenceValue}
                      onChange={(e) => setRecurrenceValue(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                      style={{
                        backgroundColor: 'var(--codex-bg)',
                        color: 'var(--codex-fg)',
                        border: '1px solid var(--codex-border)',
                      }}
                    >
                      {RECURRENCE_PRESETS.map(p => (
                        <option key={p.value} value={p.value}>{p.label}</option>
                      ))}
                    </select>
                  </div>
                  <div className="flex justify-end gap-2">
                    <button
                      onClick={() => { setEditingRecurrence(false); setRecurrenceValue(task.recurrenceRule ?? ''); }}
                      className="px-3 py-1.5 rounded text-[11px] transition-colors"
                      style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}
                    >
                      Cancel
                    </button>
                    <button
                      onClick={handleSaveRecurrence}
                      disabled={patchingField === 'recurrenceRule'}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[11px] transition-colors"
                      style={{ backgroundColor: 'var(--codex-accent)', color: 'white', opacity: patchingField === 'recurrenceRule' ? 0.7 : 1 }}
                    >
                      {patchingField === 'recurrenceRule' && <Loader2 className="w-3 h-3 animate-spin" />}
                      Save
                    </button>
                  </div>
                </div>
              ) : (
                <div className="space-y-3">
                  <div>
                    <div className="text-[11px] uppercase tracking-wider mb-1" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                      Frequency
                    </div>
                    <div className="flex items-center gap-2 text-[13px]" style={{ color: 'var(--codex-fg)' }}>
                      <RefreshCw className="w-3.5 h-3.5" strokeWidth={1.5} style={{ color: 'var(--codex-accent)' }} />
                      {rruleToLabel(task.recurrenceRule)}
                    </div>
                  </div>
                  {task.nextInstanceDate && (
                    <div>
                      <div className="text-[11px] uppercase tracking-wider mb-1" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                        Next Instance
                      </div>
                      <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>
                        {new Date(task.nextInstanceDate).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric', hour: 'numeric', minute: '2-digit' })}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {/* Time Tracking Card */}
          <div className="p-4 rounded-lg" style={{
            backgroundColor: '#141414',
            border: '1px solid var(--codex-border)'
          }}>
            <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
              Time Tracking
            </h3>
            <div className="space-y-3">
              <div className="flex items-baseline gap-3">
                <div className="text-[18px]" style={{ color: '#10a37f', fontWeight: 500 }}>
                  {formatDuration(task.totalTrackedSecs)}
                </div>
                {isTimerRunning && (
                  <div className="text-[14px]" style={{ color: '#e5c07b', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>
                    +{formatTimerDisplay(timerElapsed)}
                  </div>
                )}
              </div>

              <button
                onClick={isTimerRunning ? handleTimerStop : handleTimerStart}
                className="w-full flex items-center justify-center gap-2 py-2 rounded text-[13px] transition-colors"
                style={{
                  backgroundColor: isTimerRunning ? 'rgba(229, 80, 80, 0.2)' : 'rgba(16, 163, 127, 0.2)',
                  color: isTimerRunning ? '#e55050' : '#10a37f',
                  border: `1px solid ${isTimerRunning ? '#e5505040' : '#10a37f40'}`
                }}
              >
                {isTimerRunning ? (
                  <>
                    <Square className="w-4 h-4" strokeWidth={1.5} />
                    Stop Timer
                  </>
                ) : (
                  <>
                    <Play className="w-4 h-4" strokeWidth={1.5} />
                    Start Timer
                  </>
                )}
              </button>

              {timeEntryList.length > 0 && (
                <div className="pt-3 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                  <div className="text-[11px] mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>Recent Entries</div>
                  {timeEntryList.map(entry => (
                    <div key={entry.id} className="text-[11px] mb-2">
                      <div className="flex items-center gap-2 mb-1">
                        <span style={{ color: 'var(--codex-fg-muted)' }}>
                          {formatDateTime(entry.startedAt)}
                        </span>
                        {entry.durationSecs != null && (
                          <span style={{
                            color: 'var(--codex-fg-subtle)',
                            fontFamily: 'var(--font-mono)'
                          }}>
                            {formatDuration(entry.durationSecs)}
                          </span>
                        )}
                        <span className="px-1.5 py-0.5 rounded text-[9px]" style={{
                          backgroundColor: entry.source === 'Focus' ? 'rgba(229, 192, 123, 0.2)' : 'var(--codex-bg)',
                          color: entry.source === 'Focus' ? '#e5c07b' : 'var(--codex-fg-subtle)',
                          border: `1px solid ${entry.source === 'Focus' ? '#e5c07b40' : 'var(--codex-border)'}`
                        }}>
                          {entry.source}
                        </span>
                      </div>
                      {entry.note && (
                        <div style={{ color: 'var(--codex-fg-subtle)' }}>
                          {entry.note}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Dependencies Card */}
          {deps.data && (() => {
            const blockedBy = deps.data.blockedBy;
            const blocks = deps.data.blocks;
            const hasDeps = blockedBy.length > 0 || blocks.length > 0;

            // ── DAG layout computation ──────────────────────────────────
            const nodeW = 150;
            const nodeH = 34;
            const colGap = 200;
            const rowGap = 46;
            const padX = 20;
            const padY = 20;

            const col0Count = blockedBy.length;
            const col1Count = 1; // current task
            const col2Count = blocks.length;
            const maxRows = Math.max(col0Count, col1Count, col2Count, 1);

            const svgW = (col2Count > 0 ? 3 : col0Count > 0 ? 2 : 1) * colGap - colGap + nodeW + padX * 2;
            const svgH = maxRows * nodeH + (maxRows - 1) * (rowGap - nodeH) + padY * 2;

            const colX = (col: number) => padX + col * colGap;

            const centerY = (count: number, index: number) => {
              const totalH = count * nodeH + (count - 1) * (rowGap - nodeH);
              const startY = padY + (svgH - padY * 2 - totalH) / 2;
              return startY + index * rowGap;
            };

            type DagNode = { id: string; title: string; status: string; x: number; y: number; isCurrent: boolean };
            const nodes: DagNode[] = [];
            const edges: { sx: number; sy: number; tx: number; ty: number }[] = [];

            // Column 0: blocked by
            blockedBy.forEach((t, i) => {
              nodes.push({ id: t.id, title: t.title, status: t.status, x: colX(0), y: centerY(col0Count, i), isCurrent: false });
            });

            // Column 1: current task (always at center col — shift left if no blockers)
            const currentCol = col0Count > 0 ? 1 : 0;
            const currentX = colX(currentCol);
            const currentY = centerY(1, 0);
            nodes.push({ id: task.id, title: task.title, status: task.status, x: currentX, y: currentY, isCurrent: true });

            // Column 2: blocks
            const blocksCol = col0Count > 0 ? 2 : 1;
            blocks.forEach((t, i) => {
              nodes.push({ id: t.id, title: t.title, status: t.status, x: colX(blocksCol), y: centerY(col2Count, i), isCurrent: false });
            });

            // Edges: blocked by → current
            blockedBy.forEach((_, i) => {
              const sy = centerY(col0Count, i) + nodeH / 2;
              const sx = colX(0) + nodeW;
              const ty = currentY + nodeH / 2;
              const tx = currentX;
              edges.push({ sx, sy, tx, ty });
            });

            // Edges: current → blocks
            blocks.forEach((_, i) => {
              const sy = currentY + nodeH / 2;
              const sx = currentX + nodeW;
              const ty = centerY(col2Count, i) + nodeH / 2;
              const tx = colX(blocksCol);
              edges.push({ sx, sy, tx, ty });
            });

            return (
              <div className="p-4 rounded-lg" style={{
                backgroundColor: '#141414',
                border: '1px solid var(--codex-border)'
              }}>
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-[13px] flex items-center gap-2" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                    <GitBranch className="w-3.5 h-3.5" strokeWidth={1.5} />
                    Dependencies
                  </h3>
                </div>

                {/* Blocked By */}
                <div className="mb-3">
                  <div className="text-[11px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>Blocked By</div>
                  <div className="flex flex-wrap gap-1.5">
                    {blockedBy.length === 0 && (
                      <span className="text-[11px]" style={{ color: 'var(--codex-fg-muted)' }}>None</span>
                    )}
                    {blockedBy.map(t => (
                      <span
                        key={t.id}
                        className="group flex items-center gap-1.5 px-2 py-0.5 rounded text-[11px] transition-colors"
                        style={{
                          backgroundColor: 'var(--codex-bg)',
                          border: '1px solid var(--codex-border)',
                          cursor: 'pointer',
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                      >
                        <span
                          className="w-2 h-2 rounded-full flex-shrink-0"
                          style={{ backgroundColor: getStatusDotColor(t.status) }}
                        />
                        <span
                          onClick={() => navigate(`/tasks/${t.id}`)}
                          style={{ color: 'var(--codex-fg)' }}
                        >
                          {truncateTitle(t.title, 20)}
                        </span>
                        <button
                          onClick={(e) => { e.stopPropagation(); handleRemoveBlockedBy(t.id); }}
                          className="ml-0.5 opacity-50 hover:opacity-100 transition-opacity"
                          style={{ color: 'var(--codex-fg-subtle)' }}
                          onMouseEnter={(e) => e.currentTarget.style.color = '#ef4444'}
                          onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                        >
                          <X className="w-2.5 h-2.5" strokeWidth={2} />
                        </button>
                      </span>
                    ))}
                  </div>
                </div>

                {/* Blocks */}
                <div className="mb-3">
                  <div className="text-[11px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>Blocks</div>
                  <div className="flex flex-wrap gap-1.5">
                    {blocks.length === 0 && (
                      <span className="text-[11px]" style={{ color: 'var(--codex-fg-muted)' }}>None</span>
                    )}
                    {blocks.map(t => (
                      <span
                        key={t.id}
                        className="group flex items-center gap-1.5 px-2 py-0.5 rounded text-[11px] transition-colors"
                        style={{
                          backgroundColor: 'var(--codex-bg)',
                          border: '1px solid var(--codex-border)',
                          cursor: 'pointer',
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                      >
                        <span
                          className="w-2 h-2 rounded-full flex-shrink-0"
                          style={{ backgroundColor: getStatusDotColor(t.status) }}
                        />
                        <span
                          onClick={() => navigate(`/tasks/${t.id}`)}
                          style={{ color: 'var(--codex-fg)' }}
                        >
                          {truncateTitle(t.title, 20)}
                        </span>
                        <button
                          onClick={(e) => { e.stopPropagation(); handleRemoveBlocks(t.id); }}
                          className="ml-0.5 opacity-50 hover:opacity-100 transition-opacity"
                          style={{ color: 'var(--codex-fg-subtle)' }}
                          onMouseEnter={(e) => e.currentTarget.style.color = '#ef4444'}
                          onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                        >
                          <X className="w-2.5 h-2.5" strokeWidth={2} />
                        </button>
                      </span>
                    ))}
                  </div>
                </div>

                {/* Add Dependency */}
                <div className="relative" ref={depSearchRef}>
                  {depSearchOpen ? (
                    <div>
                      <div className="flex items-center gap-2 px-2 py-1.5 rounded border" style={{
                        backgroundColor: 'var(--codex-bg)',
                        borderColor: 'var(--codex-accent)',
                      }}>
                        <Search className="w-3 h-3 flex-shrink-0" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
                        <input
                          value={depSearchQuery}
                          onChange={(e) => setDepSearchQuery(e.target.value)}
                          autoFocus
                          placeholder="Search tasks..."
                          className="flex-1 bg-transparent outline-none text-[11px]"
                          style={{ color: 'var(--codex-fg)' }}
                          onKeyDown={(e) => {
                            if (e.key === 'Escape') { setDepSearchOpen(false); setDepSearchQuery(''); }
                          }}
                        />
                        {depSearchLoading && <Loader2 className="w-3 h-3 animate-spin" style={{ color: 'var(--codex-fg-subtle)' }} />}
                      </div>
                      {depSearchResults.length > 0 && (
                        <div className="absolute left-0 right-0 mt-1 rounded border overflow-hidden" style={{
                          backgroundColor: '#141414',
                          borderColor: 'var(--codex-border)',
                          zIndex: 10,
                          maxHeight: '200px',
                          overflowY: 'auto',
                        }}>
                          {depSearchResults.map(t => (
                            <button
                              key={t.id}
                              onClick={() => handleAddDependency(t.id)}
                              className="w-full flex items-center gap-2 px-3 py-2 text-[11px] text-left transition-colors"
                              style={{ color: 'var(--codex-fg)' }}
                              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
                              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                            >
                              <span
                                className="w-2 h-2 rounded-full flex-shrink-0"
                                style={{ backgroundColor: getStatusDotColor(t.status) }}
                              />
                              <span className="truncate">{t.title}</span>
                              <span className="ml-auto flex-shrink-0" style={{ color: 'var(--codex-fg-muted)', fontFamily: 'var(--font-mono)', fontSize: '10px' }}>
                                {t.id.slice(0, 6)}
                              </span>
                            </button>
                          ))}
                        </div>
                      )}
                      {depSearchQuery.trim() && !depSearchLoading && depSearchResults.length === 0 && (
                        <div className="absolute left-0 right-0 mt-1 px-3 py-2 rounded border text-[11px]" style={{
                          backgroundColor: '#141414',
                          borderColor: 'var(--codex-border)',
                          color: 'var(--codex-fg-muted)',
                          zIndex: 10,
                        }}>
                          No matching tasks
                        </div>
                      )}
                    </div>
                  ) : (
                    <button
                      onClick={() => setDepSearchOpen(true)}
                      className="flex items-center gap-1.5 px-2 py-1 rounded text-[11px] transition-colors"
                      style={{
                        backgroundColor: 'transparent',
                        color: 'var(--codex-fg-subtle)',
                        border: '1px solid var(--codex-border)',
                      }}
                      onMouseEnter={(e) => e.currentTarget.style.color = '#10a37f'}
                      onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                    >
                      <Plus className="w-3 h-3" strokeWidth={1.5} />
                      Add Blocker
                    </button>
                  )}
                </div>

                {/* DAG Visualization */}
                {hasDeps && (
                  <div className="mt-4 pt-3 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                    <svg
                      width={svgW}
                      height={svgH}
                      viewBox={`0 0 ${svgW} ${svgH}`}
                      style={{ display: 'block', maxWidth: '100%' }}
                    >
                      <defs>
                        <marker
                          id="arrowhead"
                          markerWidth="8"
                          markerHeight="6"
                          refX="8"
                          refY="3"
                          orient="auto"
                        >
                          <polygon points="0 0, 8 3, 0 6" fill="#444" />
                        </marker>
                      </defs>

                      {/* Edges */}
                      {edges.map((e, i) => {
                        const dx = e.tx - e.sx;
                        const cx1 = e.sx + dx * 0.6;
                        const cx2 = e.tx - dx * 0.6;
                        return (
                          <path
                            key={`edge-${i}`}
                            d={`M ${e.sx},${e.sy} C ${cx1},${e.sy} ${cx2},${e.ty} ${e.tx},${e.ty}`}
                            stroke="#444"
                            strokeWidth={1}
                            fill="none"
                            markerEnd="url(#arrowhead)"
                          />
                        );
                      })}

                      {/* Nodes */}
                      {nodes.map((node) => (
                        <g
                          key={node.id}
                          transform={`translate(${node.x}, ${node.y})`}
                          style={{ cursor: node.isCurrent ? 'default' : 'pointer' }}
                          onClick={() => { if (!node.isCurrent) navigate(`/tasks/${node.id}`); }}
                        >
                          <rect
                            width={nodeW}
                            height={nodeH}
                            rx={6}
                            fill="#1a1a1a"
                            stroke={node.isCurrent ? 'var(--codex-accent)' : 'var(--codex-border)'}
                            strokeWidth={node.isCurrent ? 1.5 : 1}
                          />
                          <circle
                            cx={14}
                            cy={nodeH / 2}
                            r={3}
                            fill={getStatusDotColor(node.status)}
                          />
                          <text
                            x={24}
                            y={nodeH / 2 + 4}
                            fontSize={11}
                            fill="var(--codex-fg)"
                          >
                            {truncateTitle(node.title, 18)}
                          </text>
                        </g>
                      ))}
                    </svg>
                  </div>
                )}
              </div>
            );
          })()}

          {/* Focus Card */}
          {task.focusedAt && (
            <div className="p-4 rounded-lg" style={{
              backgroundColor: '#141414',
              border: '1px solid var(--codex-border)'
            }}>
              <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                Focus
              </h3>
              <div className="space-y-3">
                <div className="flex justify-between text-[12px]">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>Focused since</span>
                  <span style={{ color: '#e5c07b' }}>
                    {formatDateTime(task.focusedAt)}
                  </span>
                </div>
                {task.focusDeadline && (
                  <div className="flex justify-between text-[12px]">
                    <span style={{ color: 'var(--codex-fg-subtle)' }}>Deadline</span>
                    <span style={{ color: 'var(--codex-fg)' }}>
                      {formatDateTime(task.focusDeadline)}
                    </span>
                  </div>
                )}
                <button
                  onClick={handleUnfocus}
                  className="w-full flex items-center justify-center gap-2 py-2 rounded text-[13px] transition-colors"
                  style={{
                    backgroundColor: 'rgba(229, 192, 123, 0.2)',
                    color: '#e5c07b',
                    border: '1px solid #e5c07b40'
                  }}
                >
                  <Zap className="w-4 h-4" strokeWidth={1.5} />
                  Unfocus
                </button>
              </div>
            </div>
          )}

          {/* Calendar Card */}
          {task.calendarEventUid && (
            <div className="p-4 rounded-lg" style={{
              backgroundColor: '#141414',
              border: '1px solid var(--codex-border)'
            }}>
              <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                Calendar
              </h3>
              <div className="space-y-2">
                <div className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Linked to CalDAV event
                </div>
                <div className="text-[10px] p-2 rounded" style={{
                  backgroundColor: 'var(--codex-bg)',
                  color: 'var(--codex-fg-subtle)',
                  fontFamily: 'var(--font-mono)',
                  wordBreak: 'break-all'
                }}>
                  {task.calendarEventUid}
                </div>
              </div>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}
