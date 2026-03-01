import { useState, useCallback, useMemo } from 'react';
import { Plus, Play, Pause, Trash2, Clock, CheckCircle2, AlertCircle, Loader2, AlertTriangle, X, Search, Calendar, RotateCw } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { CronJob } from '../../lib/types';

type JobStatus = 'active' | 'paused' | 'failed';
type ScheduleKind = 'cron' | 'every' | 'at';
type IntervalUnit = 'seconds' | 'minutes' | 'hours';

const CRON_PRESETS: { label: string; expr: string }[] = [
  { label: 'Every minute', expr: '* * * * *' },
  { label: 'Every 5 minutes', expr: '*/5 * * * *' },
  { label: 'Every 15 minutes', expr: '*/15 * * * *' },
  { label: 'Every 30 minutes', expr: '*/30 * * * *' },
  { label: 'Every hour', expr: '0 * * * *' },
  { label: 'Daily at midnight', expr: '0 0 * * *' },
  { label: 'Daily at 9 AM', expr: '0 9 * * *' },
  { label: 'Weekdays at 9 AM', expr: '0 9 * * MON-FRI' },
  { label: 'Weekly on Sunday', expr: '0 0 * * SUN' },
  { label: 'Monthly on 1st', expr: '0 0 1 * *' },
];

const getJobStatus = (job: CronJob): JobStatus => {
  if (job.lastStatus?.toLowerCase() === 'failed') return 'failed';
  if (!job.enabled) return 'paused';
  return 'active';
};

const getCronExpr = (schedule: unknown): string => {
  if (typeof schedule !== 'object' || !schedule) return String(schedule);
  const s = schedule as Record<string, unknown>;
  if (s.kind === 'cron' && s.expr) return String(s.expr);
  if (s.kind === 'every' && s.everyMs) {
    const ms = Number(s.everyMs);
    if (ms >= 3_600_000) return `Every ${Math.round(ms / 3_600_000)}h`;
    if (ms >= 60_000) return `Every ${Math.round(ms / 60_000)}m`;
    return `Every ${Math.round(ms / 1_000)}s`;
  }
  if (s.kind === 'at' && s.atMs) {
    const d = new Date(Number(s.atMs));
    return `Once at ${d.toLocaleString()}`;
  }
  // Legacy format: { Cron: "..." }
  if ('Cron' in s) return String(s.Cron);
  return JSON.stringify(schedule);
};

const getPayload = (payload: unknown): string => {
  if (typeof payload === 'string') return payload;
  if (typeof payload === 'object' && payload && 'message' in payload)
    return String((payload as Record<string, unknown>).message);
  return JSON.stringify(payload);
};

const formatRelativeTime = (ms: number | null): string => {
  if (ms === null) return 'Never';
  const now = Date.now();
  const diff = ms - now;
  const absDiff = Math.abs(diff);

  if (absDiff < 60_000) {
    const secs = Math.round(absDiff / 1_000);
    return diff < 0 ? `${secs}s ago` : `In ${secs}s`;
  }
  if (absDiff < 3_600_000) {
    const mins = Math.round(absDiff / 60_000);
    return diff < 0 ? `${mins}m ago` : `In ${mins}m`;
  }
  if (absDiff < 86_400_000) {
    const hours = Math.round(absDiff / 3_600_000);
    return diff < 0 ? `${hours}h ago` : `In ${hours}h`;
  }
  const days = Math.round(absDiff / 86_400_000);
  return diff < 0 ? `${days}d ago` : `In ${days}d`;
};

const DAY_NAMES: Record<string, string> = {
  '0': 'Sunday', '1': 'Monday', '2': 'Tuesday', '3': 'Wednesday',
  '4': 'Thursday', '5': 'Friday', '6': 'Saturday', '7': 'Sunday',
  SUN: 'Sunday', MON: 'Monday', TUE: 'Tuesday', WED: 'Wednesday',
  THU: 'Thursday', FRI: 'Friday', SAT: 'Saturday',
};

const formatTime12h = (hour: number, minute: number): string => {
  if (hour === 0 && minute === 0) return 'midnight';
  if (hour === 12 && minute === 0) return 'noon';
  const period = hour < 12 ? 'AM' : 'PM';
  const h = hour % 12 || 12;
  return minute === 0 ? `${h}:00 ${period}` : `${h}:${String(minute).padStart(2, '0')} ${period}`;
};

const ordinal = (n: number): string => {
  const s = ['th', 'st', 'nd', 'rd'];
  const v = n % 100;
  return n + (s[(v - 20) % 10] || s[v] || s[0]);
};

/** Convert a 5-field cron expression to human-readable text. */
const parseSchedule = (schedule: string): string => {
  // Pass through non-cron formats
  if (schedule.startsWith('Every ') || schedule.startsWith('Once at ')) return schedule;

  const parts = schedule.trim().split(/\s+/);
  if (parts.length !== 5) return schedule;

  const [min, hour, dom, _mon, dow] = parts;

  // */N * * * * → "Every N minutes"
  if (min.startsWith('*/') && hour === '*' && dom === '*' && dow === '*') {
    const n = parseInt(min.slice(2));
    if (n === 1) return 'Every minute';
    return `Every ${n} minutes`;
  }

  // 0 */N * * * → "Every N hours"
  if (min === '0' && hour.startsWith('*/') && dom === '*' && dow === '*') {
    const n = parseInt(hour.slice(2));
    if (n === 1) return 'Every hour';
    return `Every ${n} hours`;
  }

  // * * * * * → "Every minute"
  if (min === '*' && hour === '*' && dom === '*' && dow === '*') return 'Every minute';

  // Fixed minute + hour patterns
  const m = parseInt(min);
  const h = parseInt(hour);
  if (!isNaN(m) && !isNaN(h)) {
    const time = formatTime12h(h, m);

    // M H * * * → "Daily at TIME"
    if (dom === '*' && dow === '*') return `Daily at ${time}`;

    // M H * * DOW → day-specific
    if (dom === '*' && dow !== '*') {
      // Range like MON-FRI
      if (dow.includes('-')) {
        const [start, end] = dow.split('-');
        const startName = DAY_NAMES[start.toUpperCase()] ?? start;
        const endName = DAY_NAMES[end.toUpperCase()] ?? end;
        if (startName === 'Monday' && endName === 'Friday') return `Weekdays at ${time}`;
        return `${startName}–${endName} at ${time}`;
      }
      // List like 1,3,5 or MON,WED,FRI
      if (dow.includes(',')) {
        const days = dow.split(',').map(d => DAY_NAMES[d.toUpperCase()] ?? d);
        return `${days.join(', ')} at ${time}`;
      }
      // Single day
      const dayName = DAY_NAMES[dow.toUpperCase()] ?? dow;
      return `${dayName} at ${time}`;
    }

    // M H D * * → "Monthly on Dth at TIME"
    if (dow === '*' && dom !== '*') {
      const d = parseInt(dom);
      if (!isNaN(d)) return `Monthly on ${ordinal(d)} at ${time}`;
    }
  }

  // Fixed minute, wildcard hour: M * * * * → "Every hour at :MM"
  if (!isNaN(m) && hour === '*' && dom === '*' && dow === '*') {
    return m === 0 ? 'Every hour' : `Every hour at :${String(m).padStart(2, '0')}`;
  }

  return schedule;
};

export default function Cron() {
  const [searchQuery, setSearchQuery] = useState('');
  const { data: cronJobs, loading, error, setData } = useApi<CronJob[]>('/api/cron');

  // Create form state
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [newName, setNewName] = useState('');
  const [newMessage, setNewMessage] = useState('');
  const [scheduleKind, setScheduleKind] = useState<ScheduleKind>('cron');
  const [cronExpr, setCronExpr] = useState('');
  const [intervalValue, setIntervalValue] = useState('');
  const [intervalUnit, setIntervalUnit] = useState<IntervalUnit>('minutes');
  const [atDatetime, setAtDatetime] = useState('');
  const [deleteAfterRun, setDeleteAfterRun] = useState(false);

  // Delete confirmation state
  const [deletingJob, setDeletingJob] = useState<string | null>(null);

  // Run now state
  const [runningJob, setRunningJob] = useState<string | null>(null);

  const jobList = cronJobs ?? [];

  // Filter jobs by search
  const filteredJobs = useMemo(() => {
    if (!searchQuery.trim()) return jobList;
    const q = searchQuery.toLowerCase();
    return jobList.filter(j =>
      j.name.toLowerCase().includes(q) ||
      getPayload(j.payload).toLowerCase().includes(q) ||
      getCronExpr(j.schedule).toLowerCase().includes(q)
    );
  }, [jobList, searchQuery]);

  const activeCount = jobList.filter(j => j.enabled && j.lastStatus?.toLowerCase() !== 'failed').length;
  const pausedCount = jobList.filter(j => !j.enabled).length;
  const failedCount = jobList.filter(j => j.lastStatus?.toLowerCase() === 'failed').length;

  const getStatusColor = (status: JobStatus) => {
    switch (status) {
      case 'active': return 'var(--codex-accent)';
      case 'paused': return '#fbbf24';
      case 'failed': return '#ef4444';
    }
  };

  const getStatusIcon = (status: JobStatus) => {
    switch (status) {
      case 'active': return CheckCircle2;
      case 'paused': return Pause;
      case 'failed': return AlertCircle;
    }
  };

  // Toggle pause/resume
  const toggleJob = useCallback(async (jobId: string, currentEnabled: boolean) => {
    const newEnabled = !currentEnabled;

    // Optimistic update
    setData(prev =>
      prev?.map(j => j.id === jobId ? { ...j, enabled: newEnabled } : j),
    );

    try {
      const updated = await apiFetch<CronJob>(`/api/cron/${encodeURIComponent(jobId)}/toggle`, {
        method: 'PATCH',
        body: { enabled: newEnabled },
      });
      // Replace with server response for accurate nextRunAtMs etc.
      setData(prev =>
        prev?.map(j => j.id === jobId ? updated : j),
      );
    } catch {
      // Revert on failure
      setData(prev =>
        prev?.map(j => j.id === jobId ? { ...j, enabled: currentEnabled } : j),
      );
    }
  }, [setData]);

  // Delete job
  const confirmDelete = useCallback(async (jobId: string) => {
    // Optimistic update
    setData(prev => prev?.filter(j => j.id !== jobId));
    setDeletingJob(null);

    try {
      await apiFetch<void>(`/api/cron/${encodeURIComponent(jobId)}`, {
        method: 'DELETE',
      });
    } catch {
      // Refetch on failure
      const fresh = await apiFetch<CronJob[]>('/api/cron');
      setData(fresh);
    }
  }, [setData]);

  // Run job immediately
  const runJob = useCallback(async (jobId: string) => {
    setRunningJob(jobId);
    try {
      await apiFetch<void>(`/api/cron/${encodeURIComponent(jobId)}/run`, {
        method: 'POST',
      });
      // Refetch to get updated lastRunAtMs / lastStatus
      const fresh = await apiFetch<CronJob[]>('/api/cron');
      setData(fresh);
    } catch {
      // Silently fail — job may still have run
    } finally {
      setRunningJob(null);
    }
  }, [setData]);

  // Create job
  const handleCreate = useCallback(async () => {
    if (!newName.trim()) {
      setCreateError('Name is required');
      return;
    }

    // Build schedule
    let schedule: unknown;
    if (scheduleKind === 'cron') {
      if (!cronExpr.trim()) {
        setCreateError('Cron expression is required');
        return;
      }
      schedule = { kind: 'cron', expr: cronExpr.trim() };
    } else if (scheduleKind === 'every') {
      const val = parseFloat(intervalValue);
      if (!val || val <= 0) {
        setCreateError('Interval must be a positive number');
        return;
      }
      const multipliers: Record<IntervalUnit, number> = {
        seconds: 1_000,
        minutes: 60_000,
        hours: 3_600_000,
      };
      schedule = { kind: 'every', everyMs: Math.round(val * multipliers[intervalUnit]) };
    } else {
      if (!atDatetime) {
        setCreateError('Date and time is required');
        return;
      }
      schedule = { kind: 'at', atMs: new Date(atDatetime).getTime() };
    }

    // Build payload
    const payload = newMessage.trim()
      ? { kind: 'agent_turn', message: newMessage.trim(), deliver: false }
      : undefined;

    setSaving(true);
    setCreateError(null);

    try {
      const created = await apiFetch<CronJob>('/api/cron', {
        body: {
          name: newName.trim(),
          schedule,
          payload,
          deleteAfterRun,
        },
      });

      setData(prev => [...(prev ?? []), created]);

      // Reset form
      setCreating(false);
      setNewName('');
      setNewMessage('');
      setScheduleKind('cron');
      setCronExpr('');
      setIntervalValue('');
      setIntervalUnit('minutes');
      setAtDatetime('');
      setDeleteAfterRun(false);
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Failed to create job');
    } finally {
      setSaving(false);
    }
  }, [newName, newMessage, scheduleKind, cronExpr, intervalValue, intervalUnit, atDatetime, deleteAfterRun, setData]);

  const resetCreateForm = useCallback(() => {
    setCreating(false);
    setCreateError(null);
    setNewName('');
    setNewMessage('');
    setScheduleKind('cron');
    setCronExpr('');
    setIntervalValue('');
    setIntervalUnit('minutes');
    setAtDatetime('');
    setDeleteAfterRun(false);
  }, []);

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex items-center gap-3" style={{ color: 'var(--codex-fg-muted)' }}>
          <Loader2 className="w-5 h-5 animate-spin" strokeWidth={1.5} />
          <span className="text-[14px]">Loading cron jobs...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex items-center gap-3" style={{ color: '#ef4444' }}>
          <AlertTriangle className="w-5 h-5" strokeWidth={1.5} />
          <span className="text-[14px]">Failed to load cron jobs: {error.message}</span>
        </div>
      </div>
    );
  }

  const inputStyle = {
    backgroundColor: 'var(--codex-bg)',
    color: 'var(--codex-fg)',
    border: '1px solid var(--codex-border)',
  };

  const focusHandler = (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) =>
    e.currentTarget.style.borderColor = 'var(--codex-accent)';
  const blurHandler = (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) =>
    e.currentTarget.style.borderColor = 'var(--codex-border)';

  return (
    <div className="flex-1 flex flex-col overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
      {/* Header */}
      <div className="border-b px-8 py-6" style={{ borderColor: 'var(--codex-border-subtle)' }}>
        <div className="max-w-5xl mx-auto">
          <div className="flex items-center justify-between mb-6">
            <h1 className="text-xl" style={{
              color: 'var(--codex-fg)',
              fontWeight: 400
            }}>
              Cron Jobs
            </h1>

            <button
              onClick={() => setCreating(true)}
              className="flex items-center gap-2 px-4 py-2 rounded-lg transition-colors text-[14px]"
              style={{
                backgroundColor: 'var(--codex-accent)',
                color: 'white'
              }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
            >
              <Plus className="w-4 h-4" strokeWidth={1.5} />
              New Job
            </button>
          </div>

          {/* Stats + Search */}
          <div className="flex items-center justify-between">
            <div className="flex gap-6 text-[13px]">
              <div className="flex items-center gap-2">
                <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-accent)' }} />
                <span style={{ color: 'var(--codex-fg-muted)' }}>{activeCount} Active</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: '#fbbf24' }} />
                <span style={{ color: 'var(--codex-fg-muted)' }}>{pausedCount} Paused</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: '#ef4444' }} />
                <span style={{ color: 'var(--codex-fg-muted)' }}>{failedCount} Failed</span>
              </div>
            </div>

            {/* Search */}
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5" style={{ color: 'var(--codex-fg-subtle)' }} />
              <input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Filter jobs..."
                className="pl-9 pr-3 py-1.5 rounded-lg text-[12px] outline-none transition-colors w-48"
                style={inputStyle}
                onFocus={focusHandler}
                onBlur={blurHandler}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Jobs List */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-5xl mx-auto space-y-3">
          {/* Create form */}
          <AnimatePresence>
            {creating && (
              <motion.div
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                className="rounded-lg border p-6 mb-4"
                style={{
                  backgroundColor: '#141414',
                  borderColor: 'var(--codex-accent)',
                }}
              >
                <div className="flex items-center justify-between mb-5">
                  <h2 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                    Create Cron Job
                  </h2>
                  <button
                    onClick={resetCreateForm}
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  >
                    <X className="w-4 h-4" strokeWidth={1.5} />
                  </button>
                </div>

                <div className="space-y-4">
                  {/* Name */}
                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                      color: 'var(--codex-fg-subtle)', fontWeight: 500
                    }}>
                      Name *
                    </label>
                    <input
                      value={newName}
                      onChange={(e) => setNewName(e.target.value)}
                      placeholder="my-daily-task"
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                      style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }}
                      onFocus={focusHandler}
                      onBlur={blurHandler}
                    />
                  </div>

                  {/* Schedule type selector */}
                  <div>
                    <label className="block text-[11px] mb-2 uppercase tracking-wider" style={{
                      color: 'var(--codex-fg-subtle)', fontWeight: 500
                    }}>
                      Schedule Type
                    </label>
                    <div className="flex gap-2">
                      {([
                        { kind: 'cron' as const, label: 'Cron Expression', icon: Clock },
                        { kind: 'every' as const, label: 'Interval', icon: Play },
                        { kind: 'at' as const, label: 'One-time', icon: Calendar },
                      ]).map(({ kind, label, icon: Icon }) => (
                        <button
                          key={kind}
                          onClick={() => setScheduleKind(kind)}
                          className="flex items-center gap-2 px-3 py-2 rounded-lg text-[12px] transition-colors"
                          style={{
                            backgroundColor: scheduleKind === kind ? 'var(--codex-accent-dim)' : 'var(--codex-bg)',
                            color: scheduleKind === kind ? 'var(--codex-accent)' : 'var(--codex-fg-muted)',
                            border: `1px solid ${scheduleKind === kind ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                          }}
                        >
                          <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
                          {label}
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* Schedule input — varies by kind */}
                  {scheduleKind === 'cron' && (
                    <div>
                      <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                        color: 'var(--codex-fg-subtle)', fontWeight: 500
                      }}>
                        Cron Expression *
                      </label>
                      <input
                        value={cronExpr}
                        onChange={(e) => setCronExpr(e.target.value)}
                        placeholder="0 9 * * *"
                        className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors mb-2"
                        style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }}
                        onFocus={focusHandler}
                        onBlur={blurHandler}
                      />
                      <div className="flex flex-wrap gap-1.5">
                        {CRON_PRESETS.map(p => (
                          <button
                            key={p.expr}
                            onClick={() => setCronExpr(p.expr)}
                            className="px-2 py-1 rounded text-[11px] transition-colors"
                            style={{
                              backgroundColor: cronExpr === p.expr ? 'var(--codex-accent-dim)' : 'var(--codex-bg)',
                              color: cronExpr === p.expr ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
                              border: `1px solid ${cronExpr === p.expr ? 'var(--codex-accent)' : 'var(--codex-border-subtle)'}`,
                            }}
                          >
                            {p.label}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {scheduleKind === 'every' && (
                    <div>
                      <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                        color: 'var(--codex-fg-subtle)', fontWeight: 500
                      }}>
                        Run every *
                      </label>
                      <div className="flex gap-2">
                        <input
                          type="number"
                          min="1"
                          value={intervalValue}
                          onChange={(e) => setIntervalValue(e.target.value)}
                          placeholder="30"
                          className="w-24 px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        />
                        <select
                          value={intervalUnit}
                          onChange={(e) => setIntervalUnit(e.target.value as IntervalUnit)}
                          className="px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                          style={inputStyle}
                          onFocus={focusHandler}
                          onBlur={blurHandler}
                        >
                          <option value="seconds">Seconds</option>
                          <option value="minutes">Minutes</option>
                          <option value="hours">Hours</option>
                        </select>
                      </div>
                    </div>
                  )}

                  {scheduleKind === 'at' && (
                    <div>
                      <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                        color: 'var(--codex-fg-subtle)', fontWeight: 500
                      }}>
                        Run at *
                      </label>
                      <input
                        type="datetime-local"
                        value={atDatetime}
                        onChange={(e) => setAtDatetime(e.target.value)}
                        className="px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                        style={{ ...inputStyle, colorScheme: 'dark' }}
                        onFocus={focusHandler}
                        onBlur={blurHandler}
                      />
                    </div>
                  )}

                  {/* Command / message */}
                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{
                      color: 'var(--codex-fg-subtle)', fontWeight: 500
                    }}>
                      Command / Message
                    </label>
                    <input
                      value={newMessage}
                      onChange={(e) => setNewMessage(e.target.value)}
                      placeholder="Daily financial review"
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                      style={inputStyle}
                      onFocus={focusHandler}
                      onBlur={blurHandler}
                    />
                  </div>

                  {/* Delete after run toggle */}
                  <div className="flex items-center gap-3">
                    <button
                      onClick={() => setDeleteAfterRun(!deleteAfterRun)}
                      className="w-9 h-5 rounded-full relative transition-all flex-shrink-0"
                      style={{ backgroundColor: deleteAfterRun ? 'var(--codex-accent)' : '#333' }}
                    >
                      <div className="w-4 h-4 bg-white rounded-full absolute top-0.5 transition-all" style={{
                        left: deleteAfterRun ? '18px' : '2px'
                      }} />
                    </button>
                    <span className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                      Delete after first run (one-shot job)
                    </span>
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
                      style={{
                        color: 'var(--codex-fg-muted)',
                        border: '1px solid var(--codex-border)',
                      }}
                      onMouseEnter={(e) => e.currentTarget.style.borderColor = 'var(--codex-fg-subtle)'}
                      onMouseLeave={(e) => e.currentTarget.style.borderColor = 'var(--codex-border)'}
                    >
                      Cancel
                    </button>
                    <button
                      onClick={handleCreate}
                      disabled={saving}
                      className="flex items-center gap-2 px-4 py-2 rounded-lg text-[13px] transition-colors"
                      style={{
                        backgroundColor: 'var(--codex-accent)',
                        color: 'white',
                        opacity: saving ? 0.7 : 1,
                      }}
                      onMouseEnter={(e) => !saving && (e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)')}
                      onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = 'var(--codex-accent)')}
                    >
                      {saving && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                      Create Job
                    </button>
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* Empty state */}
          {filteredJobs.length === 0 && !searchQuery && (
            <div className="text-center py-16" style={{ color: 'var(--codex-fg-subtle)' }}>
              <Clock className="w-10 h-10 mx-auto mb-3" strokeWidth={1} />
              <p className="text-[14px]">No cron jobs configured</p>
              <p className="text-[12px] mt-1">Create a job to get started</p>
            </div>
          )}

          {/* No search results */}
          {filteredJobs.length === 0 && searchQuery && (
            <div className="text-center py-16" style={{ color: 'var(--codex-fg-subtle)' }}>
              <Search className="w-10 h-10 mx-auto mb-3" strokeWidth={1} />
              <p className="text-[14px]">No jobs matching "{searchQuery}"</p>
            </div>
          )}

          {/* Job cards */}
          {filteredJobs.map((job, index) => {
            const status = getJobStatus(job);
            const StatusIcon = getStatusIcon(status);
            const cronExprStr = getCronExpr(job.schedule);
            const command = getPayload(job.payload);
            return (
              <motion.div
                key={job.id}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: index * 0.05 }}
                className="p-4 rounded-lg border transition-all"
                style={{
                  backgroundColor: 'var(--codex-bg-tertiary)',
                  borderColor: 'var(--codex-border)'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--codex-bg-tertiary)';
                }}
              >
                <div className="flex items-start gap-4">
                  <div className="flex-1 min-w-0">
                    {/* Header */}
                    <div className="flex items-start justify-between gap-4 mb-3">
                      <div className="flex items-center gap-3">
                        <StatusIcon
                          className="w-5 h-5 mt-0.5"
                          strokeWidth={1.5}
                          style={{ color: getStatusColor(status) }}
                        />
                        <div>
                          <h3 className="text-[14px] mb-1" style={{
                            color: 'var(--codex-fg)',
                            fontWeight: 400
                          }}>
                            {job.name}
                          </h3>
                          <p className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                            {command}
                          </p>
                        </div>
                      </div>

                      <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide" style={{
                        backgroundColor: status === 'active' ? 'var(--codex-accent-dim)' : 'var(--codex-bg)',
                        color: getStatusColor(status),
                        border: '1px solid var(--codex-border)'
                      }}>
                        {status}
                      </span>
                    </div>

                    {/* Schedule */}
                    <div className="grid grid-cols-2 gap-6 mb-4 px-10">
                      <div>
                        <div className="text-[10px] uppercase tracking-wider mb-1" style={{
                          color: 'var(--codex-fg-subtle)',
                          fontWeight: 500
                        }}>
                          Schedule
                        </div>
                        {(() => {
                          const humanReadable = parseSchedule(cronExprStr);
                          const isTranslated = humanReadable !== cronExprStr;
                          return (
                            <>
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg-muted)' }}>
                                {humanReadable}
                              </div>
                              {isTranslated && (
                                <div className="text-[11px] mt-0.5" style={{
                                  color: 'var(--codex-fg-subtle)',
                                  fontFamily: 'var(--font-mono)'
                                }}>
                                  {cronExprStr}
                                </div>
                              )}
                            </>
                          );
                        })()}
                      </div>

                      <div>
                        <div className="text-[10px] uppercase tracking-wider mb-1" style={{
                          color: 'var(--codex-fg-subtle)',
                          fontWeight: 500
                        }}>
                          Command
                        </div>
                        <div className="text-[12px]" style={{
                          color: 'var(--codex-fg-muted)',
                          fontFamily: 'var(--font-mono)'
                        }}>
                          {command}
                        </div>
                      </div>
                    </div>

                    {/* Delete confirmation */}
                    <AnimatePresence>
                      {deletingJob === job.id && (
                        <motion.div
                          initial={{ opacity: 0, height: 0 }}
                          animate={{ opacity: 1, height: 'auto' }}
                          exit={{ opacity: 0, height: 0 }}
                          className="mx-10 mb-4 p-3 rounded-lg border"
                          style={{
                            backgroundColor: 'rgba(239, 68, 68, 0.05)',
                            borderColor: 'rgba(239, 68, 68, 0.2)',
                          }}
                        >
                          <p className="text-[12px] mb-3" style={{ color: 'var(--codex-fg-muted)' }}>
                            Delete <strong style={{ color: 'var(--codex-fg)' }}>{job.name}</strong>? This cannot be undone.
                          </p>
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => confirmDelete(job.id)}
                              className="px-3 py-1.5 rounded text-[12px] transition-colors"
                              style={{ backgroundColor: '#ef4444', color: 'white' }}
                              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#dc2626'}
                              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#ef4444'}
                            >
                              Delete
                            </button>
                            <button
                              onClick={() => setDeletingJob(null)}
                              className="px-3 py-1.5 rounded text-[12px] transition-colors"
                              style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}
                            >
                              Cancel
                            </button>
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>

                    {/* Footer */}
                    <div className="flex items-center justify-between px-10">
                      <div className="flex items-center gap-6 text-[12px]">
                        <div className="flex items-center gap-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                          <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />
                          <span>Last run: {formatRelativeTime(job.lastRunAtMs)}</span>
                        </div>
                        <div className="flex items-center gap-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                          <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />
                          <span>Next run: {!job.enabled ? 'Paused' : formatRelativeTime(job.nextRunAtMs)}</span>
                        </div>
                      </div>

                      {/* Actions */}
                      <div className="flex items-center gap-2">
                        {status === 'active' ? (
                          <button
                            onClick={() => toggleJob(job.id, job.enabled)}
                            title="Pause job"
                            className="p-1.5 rounded transition-colors"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                          >
                            <Pause className="w-4 h-4" strokeWidth={1.5} />
                          </button>
                        ) : (
                          <button
                            onClick={() => toggleJob(job.id, job.enabled)}
                            title="Resume job"
                            className="p-1.5 rounded transition-colors"
                            style={{ color: 'var(--codex-accent)' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                          >
                            <Play className="w-4 h-4" strokeWidth={1.5} />
                          </button>
                        )}
                        <button
                          onClick={() => runJob(job.id)}
                          disabled={runningJob === job.id}
                          title="Run now"
                          className="p-1.5 rounded transition-colors"
                          style={{ color: 'var(--codex-fg-subtle)' }}
                          onMouseEnter={(e) => {
                            e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                            e.currentTarget.style.color = 'var(--codex-accent)';
                          }}
                          onMouseLeave={(e) => {
                            e.currentTarget.style.backgroundColor = 'transparent';
                            e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                          }}
                        >
                          {runningJob === job.id
                            ? <Loader2 className="w-4 h-4 animate-spin" strokeWidth={1.5} />
                            : <RotateCw className="w-4 h-4" strokeWidth={1.5} />
                          }
                        </button>
                        <button
                          onClick={() => setDeletingJob(deletingJob === job.id ? null : job.id)}
                          title="Delete job"
                          className="p-1.5 rounded transition-colors"
                          style={{ color: 'var(--codex-fg-subtle)' }}
                          onMouseEnter={(e) => {
                            e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                            e.currentTarget.style.color = '#ef4444';
                          }}
                          onMouseLeave={(e) => {
                            e.currentTarget.style.backgroundColor = 'transparent';
                            e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                          }}
                        >
                          <Trash2 className="w-4 h-4" strokeWidth={1.5} />
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </motion.div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
