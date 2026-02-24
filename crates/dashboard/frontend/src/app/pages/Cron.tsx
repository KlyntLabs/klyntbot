import { useState } from 'react';
import { Plus, Play, Pause, Trash2, Clock, CheckCircle2, AlertCircle } from 'lucide-react';
import { motion } from 'motion/react';

type CronJob = {
  id: string;
  name: string;
  schedule: string;
  command: string;
  status: 'active' | 'paused' | 'failed';
  lastRun: string;
  nextRun: string;
  successRate: number;
  description: string;
};

export default function Cron() {
  const [searchQuery, setSearchQuery] = useState('');

  const cronJobs: CronJob[] = [
    {
      id: '1',
      name: 'Database Backup',
      schedule: '0 2 * * *',
      command: 'npm run backup:db',
      status: 'active',
      lastRun: '2h ago',
      nextRun: 'In 22h',
      successRate: 99.8,
      description: 'Daily backup of production database at 2:00 AM'
    },
    {
      id: '2',
      name: 'Email Digest',
      schedule: '0 9 * * MON-FRI',
      command: 'npm run email:digest',
      status: 'active',
      lastRun: '1h ago',
      nextRun: 'Tomorrow 9:00 AM',
      successRate: 100,
      description: 'Send weekly summary emails to users'
    },
    {
      id: '3',
      name: 'Cache Cleanup',
      schedule: '*/30 * * * *',
      command: 'npm run cache:clear',
      status: 'active',
      lastRun: '15m ago',
      nextRun: 'In 15m',
      successRate: 97.5,
      description: 'Clean up expired cache entries every 30 minutes'
    },
    {
      id: '4',
      name: 'API Health Check',
      schedule: '*/5 * * * *',
      command: 'npm run health:check',
      status: 'active',
      lastRun: '2m ago',
      nextRun: 'In 3m',
      successRate: 99.9,
      description: 'Monitor API endpoints every 5 minutes'
    },
    {
      id: '5',
      name: 'Log Rotation',
      schedule: '0 0 * * SUN',
      command: 'npm run logs:rotate',
      status: 'paused',
      lastRun: '3 days ago',
      nextRun: 'Paused',
      successRate: 95.2,
      description: 'Rotate and archive log files weekly'
    },
    {
      id: '6',
      name: 'Report Generation',
      schedule: '0 8 1 * *',
      command: 'npm run reports:generate',
      status: 'failed',
      lastRun: '4 days ago (failed)',
      nextRun: 'In 7 days',
      successRate: 88.5,
      description: 'Generate monthly analytics reports'
    }
  ];

  const getStatusColor = (status: CronJob['status']) => {
    switch (status) {
      case 'active': return 'var(--codex-accent)';
      case 'paused': return '#fbbf24';
      case 'failed': return '#ef4444';
    }
  };

  const getStatusIcon = (status: CronJob['status']) => {
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
              <span style={{ color: 'var(--codex-fg-muted)' }}>4 Active</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: '#fbbf24' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>1 Paused</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: '#ef4444' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>1 Failed</span>
            </div>
          </div>
        </div>
      </div>

      {/* Jobs List */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-5xl mx-auto space-y-3">
          {cronJobs.map((job, index) => {
            const StatusIcon = getStatusIcon(job.status);
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
                          style={{ color: getStatusColor(job.status) }}
                        />
                        <div>
                          <h3 className="text-[14px] mb-1" style={{
                            color: 'var(--codex-fg)',
                            fontWeight: 400
                          }}>
                            {job.name}
                          </h3>
                          <p className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                            {job.description}
                          </p>
                        </div>
                      </div>

                      <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide" style={{
                        backgroundColor: job.status === 'active' ? 'var(--codex-accent-dim)' : 'var(--codex-bg)',
                        color: getStatusColor(job.status),
                        border: '1px solid var(--codex-border)'
                      }}>
                        {job.status}
                      </span>
                    </div>

                    {/* Schedule */}
                    <div className="grid grid-cols-3 gap-6 mb-4 px-10">
                      <div>
                        <div className="text-[10px] uppercase tracking-wider mb-1" style={{
                          color: 'var(--codex-fg-subtle)',
                          fontWeight: 500
                        }}>
                          Schedule
                        </div>
                        <div className="text-[13px]" style={{ color: 'var(--codex-fg-muted)' }}>
                          {parseSchedule(job.schedule)}
                        </div>
                        <div className="text-[11px] mt-0.5" style={{
                          color: 'var(--codex-fg-subtle)',
                          fontFamily: 'var(--font-mono)'
                        }}>
                          {job.schedule}
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
                          {job.command}
                        </div>
                      </div>

                      <div>
                        <div className="text-[10px] uppercase tracking-wider mb-1" style={{
                          color: 'var(--codex-fg-subtle)',
                          fontWeight: 500
                        }}>
                          Success Rate
                        </div>
                        <div className="text-[13px]" style={{
                          color: job.successRate > 95 ? 'var(--codex-accent)' : job.successRate > 90 ? '#fbbf24' : '#ef4444'
                        }}>
                          {job.successRate}%
                        </div>
                      </div>
                    </div>

                    {/* Footer */}
                    <div className="flex items-center justify-between px-10">
                      <div className="flex items-center gap-6 text-[12px]">
                        <div className="flex items-center gap-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                          <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />
                          <span>Last run: {job.lastRun}</span>
                        </div>
                        <div className="flex items-center gap-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                          <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />
                          <span>Next run: {job.nextRun}</span>
                        </div>
                      </div>

                      {/* Actions */}
                      <div className="flex items-center gap-2">
                        {job.status === 'active' ? (
                          <button
                            className="p-1.5 rounded transition-colors"
                            style={{ color: 'var(--codex-fg-subtle)' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                          >
                            <Pause className="w-4 h-4" strokeWidth={1.5} />
                          </button>
                        ) : (
                          <button
                            className="p-1.5 rounded transition-colors"
                            style={{ color: 'var(--codex-accent)' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                          >
                            <Play className="w-4 h-4" strokeWidth={1.5} />
                          </button>
                        )}
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
