import { useState } from 'react';
import { Plus, Play, Pause, Trash2, Clock, CheckCircle2, AlertCircle, Loader2, AlertTriangle } from 'lucide-react';
import { motion } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import type { CronJob } from '../../lib/types';

type JobStatus = 'active' | 'paused' | 'failed';

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

export default function Cron() {
  const [searchQuery, setSearchQuery] = useState('');
  const { data: cronJobs, loading, error } = useApi<CronJob[]>('/api/cron');

  const jobList = cronJobs ?? [];

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

  const parseSchedule = (schedule: string) => {
    if (schedule === '0 2 * * *') return 'Daily at 2:00 AM';
    if (schedule === '0 9 * * MON-FRI') return 'Weekdays at 9:00 AM';
    if (schedule === '*/30 * * * *') return 'Every 30 minutes';
    if (schedule === '*/5 * * * *') return 'Every 5 minutes';
    if (schedule === '0 0 * * SUN') return 'Weekly on Sunday';
    if (schedule === '0 8 1 * *') return 'Monthly on 1st at 8:00 AM';
    return schedule;
  };

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

            {/* TODO: Implement create cron job */}
            <button
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

          {/* Stats */}
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
        </div>
      </div>

      {/* Jobs List */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-5xl mx-auto space-y-3">
          {jobList.length === 0 && (
            <div className="text-center py-16" style={{ color: 'var(--codex-fg-subtle)' }}>
              <Clock className="w-10 h-10 mx-auto mb-3" strokeWidth={1} />
              <p className="text-[14px]">No cron jobs configured</p>
              <p className="text-[12px] mt-1">Create a job to get started</p>
            </div>
          )}
          {jobList.map((job, index) => {
            const status = getJobStatus(job);
            const StatusIcon = getStatusIcon(status);
            const cronExpr = getCronExpr(job.schedule);
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
                        <div className="text-[13px]" style={{ color: 'var(--codex-fg-muted)' }}>
                          {parseSchedule(cronExpr)}
                        </div>
                        <div className="text-[11px] mt-0.5" style={{
                          color: 'var(--codex-fg-subtle)',
                          fontFamily: 'var(--font-mono)'
                        }}>
                          {cronExpr}
                        </div>
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
                          // TODO: Implement pause cron job
                          <button
                            className="p-1.5 rounded transition-colors"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                          >
                            <Pause className="w-4 h-4" strokeWidth={1.5} />
                          </button>
                        ) : (
                          // TODO: Implement resume cron job
                          <button
                            className="p-1.5 rounded transition-colors"
                            style={{ color: 'var(--codex-accent)' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                          >
                            <Play className="w-4 h-4" strokeWidth={1.5} />
                          </button>
                        )}
                        {/* TODO: Implement delete cron job */}
                        <button
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
