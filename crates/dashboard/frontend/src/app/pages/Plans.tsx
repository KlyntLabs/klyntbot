import { useState, useCallback, useMemo } from 'react';
import {
  Clock, ChevronRight, Plus, Loader2,
  AlertTriangle, Search, X,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useNavigate } from 'react-router';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Plan } from '../../lib/types';

// ── Helpers ──────────────────────────────────────────────────────────────────

function formatRelativeTime(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays > 0) return `${diffDays} day${diffDays === 1 ? '' : 's'} ago`;
  if (diffHours > 0) return `${diffHours} hour${diffHours === 1 ? '' : 's'} ago`;
  if (diffMins > 0) return `${diffMins} min${diffMins === 1 ? '' : 's'} ago`;
  return 'just now';
}

function getStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'executing': return 'var(--codex-accent)';
    case 'approved': return 'var(--codex-accent)';
    case 'draft': return '#fbbf24';
    case 'failed': return '#ef4444';
    case 'abandoned': return 'var(--codex-fg-subtle)';
    default: return 'var(--codex-fg-subtle)';
  }
}

function isActiveStatus(status: string): boolean {
  const s = status.toLowerCase();
  return s === 'executing' || s === 'approved';
}

function getVisibilityColor(visibility: string): string {
  switch (visibility) {
    case 'silent': return '#6b7280';
    case 'on_failure': return '#f59e0b';
    default: return '';
  }
}

// ── Component ────────────────────────────────────────────────────────────────

export default function Plans() {
  const [searchQuery, setSearchQuery] = useState('');

  // Create form state
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newIterationLimit, setNewIterationLimit] = useState('20');

  const navigate = useNavigate();
  const { data: plans, loading, error, setData } = useApi<Plan[]>('/api/plans');
  const planList = plans ?? [];

  // Filter plans by search
  const filteredPlans = useMemo(() => {
    if (!searchQuery.trim()) return planList;
    const q = searchQuery.toLowerCase();
    return planList.filter(p =>
      p.title.toLowerCase().includes(q) ||
      p.description.toLowerCase().includes(q) ||
      p.status.toLowerCase().includes(q) ||
      (p.visibility && p.visibility.toLowerCase().includes(q))
    );
  }, [planList, searchQuery]);

  // Stats
  const activeCount = planList.filter(p => { const s = p.status.toLowerCase(); return s === 'executing' || s === 'approved'; }).length;
  const draftCount = planList.filter(p => p.status.toLowerCase() === 'draft').length;
  const completedCount = planList.filter(p => p.status.toLowerCase() === 'completed').length;

  // ── Create Plan ──────────────────────────────────────────────────────────

  const handleCreate = useCallback(async () => {
    if (!newTitle.trim()) {
      setCreateError('Title is required');
      return;
    }

    setSaving(true);
    setCreateError(null);

    try {
      const created = await apiFetch<Plan>('/api/plans', {
        body: {
          title: newTitle.trim(),
          description: newDescription.trim() || undefined,
          iterationLimit: parseInt(newIterationLimit) || 20,
        },
      });

      setData(prev => [created, ...(prev ?? [])]);
      setCreating(false);
      setNewTitle('');
      setNewDescription('');
      setNewIterationLimit('20');
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : 'Failed to create plan');
    } finally {
      setSaving(false);
    }
  }, [newTitle, newDescription, newIterationLimit, setData]);

  const resetCreateForm = useCallback(() => {
    setCreating(false);
    setCreateError(null);
    setNewTitle('');
    setNewDescription('');
    setNewIterationLimit('20');
  }, []);

  // ── Shared styles ────────────────────────────────────────────────────────

  const inputStyle = {
    backgroundColor: 'var(--codex-bg)',
    color: 'var(--codex-fg)',
    border: '1px solid var(--codex-border)',
  };

  const focusHandler = (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement>) =>
    e.currentTarget.style.borderColor = 'var(--codex-accent)';
  const blurHandler = (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement>) =>
    e.currentTarget.style.borderColor = 'var(--codex-border)';

  // ── Loading / Error states ───────────────────────────────────────────────

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex items-center gap-3" style={{ color: 'var(--codex-fg-muted)' }}>
          <Loader2 className="w-5 h-5 animate-spin" strokeWidth={1.5} />
          <span className="text-[14px]">Loading plans...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex items-center gap-3" style={{ color: '#ef4444' }}>
          <AlertTriangle className="w-5 h-5" strokeWidth={1.5} />
          <span className="text-[14px]">Failed to load plans: {error.message}</span>
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
            <h1 className="text-xl" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
              Plans
            </h1>

            <button
              onClick={() => setCreating(true)}
              className="flex items-center gap-2 px-4 py-2 rounded-lg transition-colors text-[14px]"
              style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
            >
              <Plus className="w-4 h-4" strokeWidth={1.5} />
              New Plan
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
                <span style={{ color: 'var(--codex-fg-muted)' }}>{draftCount} Draft</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-fg-subtle)' }} />
                <span style={{ color: 'var(--codex-fg-muted)' }}>{completedCount} Completed</span>
              </div>
            </div>

            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5" style={{ color: 'var(--codex-fg-subtle)' }} />
              <input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Filter plans..."
                className="pl-9 pr-3 py-1.5 rounded-lg text-[12px] outline-none transition-colors w-48"
                style={inputStyle}
                onFocus={focusHandler}
                onBlur={blurHandler}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Plans List */}
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
                style={{ backgroundColor: '#141414', borderColor: 'var(--codex-accent)' }}
              >
                <div className="flex items-center justify-between mb-5">
                  <h2 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                    Create Plan
                  </h2>
                  <button onClick={resetCreateForm} style={{ color: 'var(--codex-fg-subtle)' }}>
                    <X className="w-4 h-4" strokeWidth={1.5} />
                  </button>
                </div>

                <div className="space-y-4">
                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                      Title *
                    </label>
                    <input
                      value={newTitle}
                      onChange={(e) => setNewTitle(e.target.value)}
                      placeholder="Plan title"
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                      style={inputStyle}
                      onFocus={focusHandler}
                      onBlur={blurHandler}
                      onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
                    />
                  </div>

                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                      Description
                    </label>
                    <textarea
                      value={newDescription}
                      onChange={(e) => setNewDescription(e.target.value)}
                      placeholder="What should this plan accomplish?"
                      rows={3}
                      className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors resize-none"
                      style={inputStyle}
                      onFocus={focusHandler}
                      onBlur={blurHandler}
                    />
                  </div>

                  <div>
                    <label className="block text-[11px] mb-1.5 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                      Iteration Limit
                    </label>
                    <input
                      type="number"
                      min="1"
                      max="100"
                      value={newIterationLimit}
                      onChange={(e) => setNewIterationLimit(e.target.value)}
                      className="w-24 px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                      style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }}
                      onFocus={focusHandler}
                      onBlur={blurHandler}
                    />
                    <span className="ml-2 text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                      Max LLM iterations per step
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
                      style={{
                        backgroundColor: 'var(--codex-accent)',
                        color: 'white',
                        opacity: saving ? 0.7 : 1,
                      }}
                      onMouseEnter={(e) => !saving && (e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)')}
                      onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = 'var(--codex-accent)')}
                    >
                      {saving && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                      Create Plan
                    </button>
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* Empty state */}
          {filteredPlans.length === 0 && !searchQuery && (
            <div className="text-center py-16" style={{ color: 'var(--codex-fg-subtle)' }}>
              <Clock className="w-10 h-10 mx-auto mb-3" strokeWidth={1} />
              <p className="text-[14px]">No plans yet</p>
              <p className="text-[12px] mt-1">Create a plan to get started</p>
            </div>
          )}

          {/* No search results */}
          {filteredPlans.length === 0 && searchQuery && (
            <div className="text-center py-16" style={{ color: 'var(--codex-fg-subtle)' }}>
              <Search className="w-10 h-10 mx-auto mb-3" strokeWidth={1} />
              <p className="text-[14px]">No plans matching "{searchQuery}"</p>
            </div>
          )}

          {/* Plan cards */}
          {filteredPlans.map((plan, index) => (
            <motion.div
              key={plan.id}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: index * 0.05 }}
              className="rounded-lg border"
              style={{
                backgroundColor: 'var(--codex-bg-tertiary)',
                borderColor: 'var(--codex-border)',
              }}
            >
              <button
                onClick={() => navigate(`/plans/${plan.id}`)}
                className="w-full p-5 text-left transition-colors"
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
              >
                <div className="flex items-start gap-4">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-3 mb-2">
                      <h3 className="text-[15px]" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
                        {plan.title}
                      </h3>
                      <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide" style={{
                        backgroundColor: isActiveStatus(plan.status) ? 'var(--codex-accent-dim)' : 'var(--codex-bg)',
                        color: getStatusColor(plan.status),
                        border: '1px solid var(--codex-border)',
                      }}>
                        {plan.status}
                      </span>
                      {plan.visibility && plan.visibility !== 'transparent' && (
                        <span className="px-2 py-0.5 rounded text-[10px] tracking-wide" style={{
                          backgroundColor: 'var(--codex-bg)',
                          color: getVisibilityColor(plan.visibility),
                          border: '1px solid var(--codex-border)',
                        }}>
                          {plan.visibility === 'silent' ? 'silent' : 'on failure'}
                        </span>
                      )}
                    </div>

                    {plan.description && (
                      <p className="text-[13px] mb-4" style={{ color: 'var(--codex-fg-subtle)' }}>
                        {plan.description}
                      </p>
                    )}

                    {/* Progress */}
                    {plan.currentStepIndex > 0 && (
                      <div className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>
                        Step {plan.currentStepIndex}
                      </div>
                    )}

                    {/* Metadata row */}
                    <div className="flex items-center gap-4 mt-3 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                      <div className="flex items-center gap-2">
                        <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />
                        {formatRelativeTime(plan.createdAt)}
                      </div>
                      <span style={{ fontFamily: 'var(--font-mono)' }}>
                        Limit: {plan.iterationLimit}
                      </span>
                      {plan.completedAt && (
                        <span>
                          Completed {formatRelativeTime(plan.completedAt)}
                        </span>
                      )}
                    </div>
                  </div>

                  <ChevronRight
                    className="w-5 h-5 mt-1"
                    strokeWidth={1.5}
                    style={{ color: 'var(--codex-fg-subtle)' }}
                  />
                </div>
              </button>
            </motion.div>
          ))}
        </div>
      </div>
    </div>
  );
}
