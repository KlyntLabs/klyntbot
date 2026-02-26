import { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import {
  CheckCircle2, Circle, Clock, ChevronRight, Plus, Loader2,
  AlertTriangle, XCircle, SkipForward, Search, X, Trash2,
  Play, Ban, Check, Pencil,
} from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Plan, PlanWithSteps, PlanStep } from '../../lib/types';

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

function getStepStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'completed': return 'var(--codex-accent)';
    case 'executing': return 'var(--codex-accent)';
    case 'failed': return '#ef4444';
    default: return 'var(--codex-fg-subtle)';
  }
}

function computeProgress(steps: PlanStep[]): { completedSteps: number; totalSteps: number; progress: number } {
  const totalSteps = steps.length;
  const completedSteps = steps.filter((s) => s.status.toLowerCase() === 'completed').length;
  const progress = totalSteps > 0 ? Math.round((completedSteps / totalSteps) * 100) : 0;
  return { completedSteps, totalSteps, progress };
}

/** Which actions are available for a given plan status. */
function getAvailableActions(status: string): string[] {
  switch (status.toLowerCase()) {
    case 'draft': return ['approve', 'abandon', 'edit', 'delete'];
    case 'approved': return ['execute', 'abandon', 'edit', 'delete'];
    case 'executing': return ['abandon'];
    case 'completed':
    case 'failed':
    case 'abandoned':
      return ['delete'];
    default: return [];
  }
}

// ── Component ────────────────────────────────────────────────────────────────

export default function Plans() {
  const [expandedPlan, setExpandedPlan] = useState<string | null>(null);
  const [expandedPlanSteps, setExpandedPlanSteps] = useState<Record<string, PlanStep[]>>({});
  const [stepsLoading, setStepsLoading] = useState<Record<string, boolean>>({});
  const [searchQuery, setSearchQuery] = useState('');

  // Create form state
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState('');
  const [newDescription, setNewDescription] = useState('');
  const [newIterationLimit, setNewIterationLimit] = useState('20');

  // Edit form state
  const [editingPlan, setEditingPlan] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [editDescription, setEditDescription] = useState('');
  const [editIterationLimit, setEditIterationLimit] = useState('');
  const [editSaving, setEditSaving] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);

  // Delete confirmation state
  const [deletingPlan, setDeletingPlan] = useState<string | null>(null);

  // Status transition loading
  const [transitioningPlan, setTransitioningPlan] = useState<string | null>(null);

  const { data: plans, loading, error, setData } = useApi<Plan[]>('/api/plans');
  const planList = plans ?? [];

  // Filter plans by search
  const filteredPlans = useMemo(() => {
    if (!searchQuery.trim()) return planList;
    const q = searchQuery.toLowerCase();
    return planList.filter(p =>
      p.title.toLowerCase().includes(q) ||
      p.description.toLowerCase().includes(q) ||
      p.status.toLowerCase().includes(q)
    );
  }, [planList, searchQuery]);

  // Stats
  const activeCount = planList.filter(p => { const s = p.status.toLowerCase(); return s === 'executing' || s === 'approved'; }).length;
  const draftCount = planList.filter(p => p.status.toLowerCase() === 'draft').length;
  const completedCount = planList.filter(p => p.status.toLowerCase() === 'completed').length;

  // ── Expand/Collapse ──────────────────────────────────────────────────────

  const handleTogglePlan = useCallback(async (planId: string) => {
    if (expandedPlan === planId) {
      setExpandedPlan(null);
      return;
    }

    setExpandedPlan(planId);

    if (!expandedPlanSteps[planId]) {
      setStepsLoading(prev => ({ ...prev, [planId]: true }));
      try {
        const result = await apiFetch<PlanWithSteps>('/api/plans/' + planId);
        setExpandedPlanSteps(prev => ({ ...prev, [planId]: result.steps }));
      } catch {
        setExpandedPlanSteps(prev => ({ ...prev, [planId]: [] }));
      } finally {
        setStepsLoading(prev => ({ ...prev, [planId]: false }));
      }
    }
  }, [expandedPlan, expandedPlanSteps]);

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

  // ── Edit Plan ────────────────────────────────────────────────────────────

  const startEditing = useCallback((plan: Plan) => {
    setEditingPlan(plan.id);
    setEditTitle(plan.title);
    setEditDescription(plan.description);
    setEditIterationLimit(String(plan.iterationLimit));
    setEditError(null);
  }, []);

  const handleEdit = useCallback(async () => {
    if (!editingPlan) return;
    if (!editTitle.trim()) {
      setEditError('Title is required');
      return;
    }

    setEditSaving(true);
    setEditError(null);

    try {
      const updated = await apiFetch<Plan>(`/api/plans/${editingPlan}`, {
        method: 'PATCH',
        body: {
          title: editTitle.trim(),
          description: editDescription.trim(),
          iterationLimit: parseInt(editIterationLimit) || 20,
        },
      });

      setData(prev => prev?.map(p => p.id === editingPlan ? { ...p, ...updated } : p));
      setEditingPlan(null);
    } catch (err) {
      setEditError(err instanceof Error ? err.message : 'Failed to update plan');
    } finally {
      setEditSaving(false);
    }
  }, [editingPlan, editTitle, editDescription, editIterationLimit, setData]);

  // ── Delete Plan ──────────────────────────────────────────────────────────

  const confirmDelete = useCallback(async (planId: string) => {
    setData(prev => prev?.filter(p => p.id !== planId));
    setDeletingPlan(null);
    if (expandedPlan === planId) setExpandedPlan(null);

    try {
      await apiFetch<void>(`/api/plans/${planId}`, { method: 'DELETE' });
    } catch {
      const fresh = await apiFetch<Plan[]>('/api/plans');
      setData(fresh);
    }
  }, [setData, expandedPlan]);

  // ── Status Transitions ───────────────────────────────────────────────────

  const transitionStatus = useCallback(async (planId: string, newStatus: string) => {
    setTransitioningPlan(planId);

    try {
      await apiFetch<{ status: string }>(`/api/plans/${planId}/status`, {
        method: 'POST',
        body: { status: newStatus },
      });

      setData(prev => prev?.map(p => p.id === planId ? { ...p, status: newStatus } : p));
    } catch (err) {
      console.error('Status transition failed:', err);
    } finally {
      setTransitioningPlan(null);
    }
  }, [setData]);

  // ── Poll executing plan steps ───────────────────────────────────────────
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    // If an expanded plan is "executing", poll its steps every 3s
    if (expandedPlan) {
      const plan = planList.find(p => p.id === expandedPlan);
      if (plan && plan.status.toLowerCase() === 'executing') {
        pollRef.current = setInterval(async () => {
          try {
            const result = await apiFetch<PlanWithSteps>('/api/plans/' + expandedPlan);
            setExpandedPlanSteps(prev => ({ ...prev, [expandedPlan]: result.steps }));
            // If plan status changed, update the list too
            if (result.plan.status.toLowerCase() !== 'executing') {
              setData(prev => prev?.map(p => p.id === expandedPlan ? { ...p, status: result.plan.status } : p));
            }
          } catch {
            // Ignore poll errors
          }
        }, 3000);
      }
    }
    return () => {
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [expandedPlan, planList, setData]);

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
          {filteredPlans.map((plan, index) => {
            const steps = expandedPlanSteps[plan.id] ?? [];
            const { completedSteps, totalSteps, progress } = steps.length > 0
              ? computeProgress(steps)
              : { completedSteps: plan.currentStepIndex, totalSteps: 0, progress: 0 };
            const actions = getAvailableActions(plan.status);
            const isTransitioning = transitioningPlan === plan.id;
            const isEditing = editingPlan === plan.id;

            return (
              <motion.div
                key={plan.id}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: index * 0.05 }}
                className="rounded-lg border"
                style={{
                  backgroundColor: 'var(--codex-bg-tertiary)',
                  borderColor: isEditing ? 'var(--codex-accent)' : 'var(--codex-border)',
                }}
              >
                {/* Edit form (inline) */}
                <AnimatePresence>
                  {isEditing && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className="p-5 border-b"
                      style={{ borderColor: 'var(--codex-border)' }}
                    >
                      <div className="flex items-center justify-between mb-4">
                        <h3 className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                          Edit Plan
                        </h3>
                        <button onClick={() => setEditingPlan(null)} style={{ color: 'var(--codex-fg-subtle)' }}>
                          <X className="w-4 h-4" strokeWidth={1.5} />
                        </button>
                      </div>

                      <div className="space-y-3">
                        <div>
                          <label className="block text-[11px] mb-1 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                            Title
                          </label>
                          <input
                            value={editTitle}
                            onChange={(e) => setEditTitle(e.target.value)}
                            className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                            style={inputStyle}
                            onFocus={focusHandler}
                            onBlur={blurHandler}
                            onKeyDown={(e) => e.key === 'Enter' && handleEdit()}
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] mb-1 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                            Description
                          </label>
                          <textarea
                            value={editDescription}
                            onChange={(e) => setEditDescription(e.target.value)}
                            rows={2}
                            className="w-full px-3 py-2 rounded-lg text-[13px] outline-none transition-colors resize-none"
                            style={inputStyle}
                            onFocus={focusHandler}
                            onBlur={blurHandler}
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] mb-1 uppercase tracking-wider" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>
                            Iteration Limit
                          </label>
                          <input
                            type="number"
                            min="1"
                            max="100"
                            value={editIterationLimit}
                            onChange={(e) => setEditIterationLimit(e.target.value)}
                            className="w-24 px-3 py-2 rounded-lg text-[13px] outline-none transition-colors"
                            style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }}
                            onFocus={focusHandler}
                            onBlur={blurHandler}
                          />
                        </div>

                        {editError && (
                          <div className="text-[12px] flex items-center gap-1.5" style={{ color: '#ef4444' }}>
                            <AlertTriangle className="w-3.5 h-3.5" strokeWidth={1.5} />
                            {editError}
                          </div>
                        )}

                        <div className="flex justify-end gap-2 pt-1">
                          <button
                            onClick={() => setEditingPlan(null)}
                            className="px-3 py-1.5 rounded-lg text-[12px] transition-colors"
                            style={{ color: 'var(--codex-fg-muted)', border: '1px solid var(--codex-border)' }}
                          >
                            Cancel
                          </button>
                          <button
                            onClick={handleEdit}
                            disabled={editSaving}
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[12px] transition-colors"
                            style={{
                              backgroundColor: 'var(--codex-accent)',
                              color: 'white',
                              opacity: editSaving ? 0.7 : 1,
                            }}
                          >
                            {editSaving && <Loader2 className="w-3 h-3 animate-spin" />}
                            Save
                          </button>
                        </div>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>

                {/* Plan Header (clickable to expand) */}
                <button
                  onClick={() => handleTogglePlan(plan.id)}
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
                      </div>

                      {plan.description && (
                        <p className="text-[13px] mb-4" style={{ color: 'var(--codex-fg-subtle)' }}>
                          {plan.description}
                        </p>
                      )}

                      {/* Progress */}
                      <div className="space-y-2">
                        <div className="flex items-center justify-between text-[12px]">
                          <span style={{ color: 'var(--codex-fg-muted)' }}>
                            Step {completedSteps}/{totalSteps}
                          </span>
                          <span style={{ color: 'var(--codex-fg-muted)' }}>{progress}%</span>
                        </div>
                        <div className="h-1 rounded-full overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
                          <div
                            className="h-full rounded-full transition-all"
                            style={{ width: `${progress}%`, backgroundColor: 'var(--codex-accent)' }}
                          />
                        </div>
                      </div>

                      <div className="flex items-center gap-2 mt-3 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                        <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />
                        {formatRelativeTime(plan.createdAt)}
                      </div>
                    </div>

                    <ChevronRight
                      className="w-5 h-5 transition-transform mt-1"
                      strokeWidth={1.5}
                      style={{
                        color: 'var(--codex-fg-subtle)',
                        transform: expandedPlan === plan.id ? 'rotate(90deg)' : 'rotate(0deg)',
                      }}
                    />
                  </div>
                </button>

                {/* Action buttons + Delete confirmation */}
                <div className="border-t px-5 py-3" style={{ borderColor: 'var(--codex-border)' }}>
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-4 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                      <span style={{ fontFamily: 'var(--font-mono)' }}>
                        Limit: {plan.iterationLimit}
                      </span>
                      {plan.completedAt && (
                        <span>
                          Completed {formatRelativeTime(plan.completedAt)}
                        </span>
                      )}
                    </div>

                    {/* Action buttons */}
                    <div className="flex items-center gap-1.5">
                      {actions.includes('approve') && (
                        <button
                          onClick={(e) => { e.stopPropagation(); transitionStatus(plan.id, 'approved'); }}
                          disabled={isTransitioning}
                          title="Approve plan"
                          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded text-[11px] transition-colors"
                          style={{
                            color: 'var(--codex-accent)',
                            border: '1px solid var(--codex-border)',
                          }}
                          onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-accent-dim)'; e.currentTarget.style.borderColor = 'var(--codex-accent)'; }}
                          onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.borderColor = 'var(--codex-border)'; }}
                        >
                          {isTransitioning ? <Loader2 className="w-3 h-3 animate-spin" /> : <Check className="w-3 h-3" strokeWidth={2} />}
                          Approve
                        </button>
                      )}
                      {actions.includes('execute') && (
                        <button
                          onClick={(e) => { e.stopPropagation(); transitionStatus(plan.id, 'executing'); }}
                          disabled={isTransitioning}
                          title="Execute plan"
                          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded text-[11px] transition-colors"
                          style={{
                            color: 'var(--codex-accent)',
                            border: '1px solid var(--codex-border)',
                          }}
                          onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-accent-dim)'; e.currentTarget.style.borderColor = 'var(--codex-accent)'; }}
                          onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.borderColor = 'var(--codex-border)'; }}
                        >
                          {isTransitioning ? <Loader2 className="w-3 h-3 animate-spin" /> : <Play className="w-3 h-3" strokeWidth={2} />}
                          Execute
                        </button>
                      )}
                      {actions.includes('abandon') && (
                        <button
                          onClick={(e) => { e.stopPropagation(); transitionStatus(plan.id, 'abandoned'); }}
                          disabled={isTransitioning}
                          title="Abandon plan"
                          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded text-[11px] transition-colors"
                          style={{
                            color: 'var(--codex-fg-subtle)',
                            border: '1px solid var(--codex-border)',
                          }}
                          onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; e.currentTarget.style.color = '#ef4444'; }}
                          onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'var(--codex-fg-subtle)'; }}
                        >
                          <Ban className="w-3 h-3" strokeWidth={2} />
                          Abandon
                        </button>
                      )}
                      {actions.includes('edit') && (
                        <button
                          onClick={(e) => { e.stopPropagation(); startEditing(plan); }}
                          title="Edit plan"
                          className="p-1.5 rounded transition-colors"
                          style={{ color: 'var(--codex-fg-subtle)' }}
                          onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg)'}
                          onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                        >
                          <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />
                        </button>
                      )}
                      {actions.includes('delete') && (
                        <button
                          onClick={(e) => { e.stopPropagation(); setDeletingPlan(deletingPlan === plan.id ? null : plan.id); }}
                          title="Delete plan"
                          className="p-1.5 rounded transition-colors"
                          style={{ color: 'var(--codex-fg-subtle)' }}
                          onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; e.currentTarget.style.color = '#ef4444'; }}
                          onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'var(--codex-fg-subtle)'; }}
                        >
                          <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Delete confirmation */}
                  <AnimatePresence>
                    {deletingPlan === plan.id && (
                      <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: 'auto' }}
                        exit={{ opacity: 0, height: 0 }}
                        className="mt-3 p-3 rounded-lg border"
                        style={{
                          backgroundColor: 'rgba(239, 68, 68, 0.05)',
                          borderColor: 'rgba(239, 68, 68, 0.2)',
                        }}
                      >
                        <p className="text-[12px] mb-3" style={{ color: 'var(--codex-fg-muted)' }}>
                          Delete <strong style={{ color: 'var(--codex-fg)' }}>{plan.title}</strong>? This will also delete all steps.
                        </p>
                        <div className="flex items-center gap-2">
                          <button
                            onClick={(e) => { e.stopPropagation(); confirmDelete(plan.id); }}
                            className="px-3 py-1.5 rounded text-[12px] transition-colors"
                            style={{ backgroundColor: '#ef4444', color: 'white' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#dc2626'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#ef4444'}
                          >
                            Delete
                          </button>
                          <button
                            onClick={(e) => { e.stopPropagation(); setDeletingPlan(null); }}
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

                {/* Plan Steps (expanded) */}
                <AnimatePresence>
                  {expandedPlan === plan.id && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className="border-t px-5 py-4"
                      style={{ borderColor: 'var(--codex-border)' }}
                    >
                      {stepsLoading[plan.id] ? (
                        <div className="flex items-center justify-center py-4">
                          <Loader2
                            className="w-4 h-4 animate-spin"
                            strokeWidth={1.5}
                            style={{ color: 'var(--codex-fg-subtle)' }}
                          />
                        </div>
                      ) : steps.length === 0 ? (
                        <div className="flex items-center justify-center py-4">
                          <span className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                            No steps defined
                          </span>
                        </div>
                      ) : (
                        <div className="space-y-1">
                          {steps.map((step, stepIndex) => (
                            <motion.div
                              key={step.id}
                              initial={{ opacity: 0, x: -10 }}
                              animate={{ opacity: 1, x: 0 }}
                              transition={{ delay: stepIndex * 0.04 }}
                              className="flex items-start gap-3 p-3 rounded-lg transition-colors"
                              style={{ backgroundColor: 'transparent' }}
                              onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'}
                              onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                            >
                              {/* Step status icon */}
                              <div className="mt-0.5">
                                {step.status.toLowerCase() === 'completed' ? (
                                  <CheckCircle2 className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                                ) : step.status.toLowerCase() === 'executing' ? (
                                  <Loader2 className="w-4 h-4 animate-spin" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                                ) : step.status.toLowerCase() === 'failed' ? (
                                  <XCircle className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                                ) : step.status.toLowerCase() === 'skipped' ? (
                                  <SkipForward className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                                ) : (
                                  <Circle className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                                )}
                              </div>

                              {/* Step content */}
                              <div className="flex-1 min-w-0">
                                <span className="text-[13px] block" style={{
                                  color: step.status.toLowerCase() === 'completed' ? 'var(--codex-fg-subtle)' : 'var(--codex-fg)',
                                  textDecoration: step.status.toLowerCase() === 'completed' ? 'line-through' : 'none',
                                }}>
                                  {step.description}
                                </span>

                                {/* Step details row */}
                                <div className="flex items-center gap-3 mt-1.5 text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                                  {step.reasoning && (
                                    <span className="truncate max-w-xs" title={step.reasoning}>
                                      {step.reasoning}
                                    </span>
                                  )}
                                  {step.expectedTools.length > 0 && (
                                    <span style={{ fontFamily: 'var(--font-mono)' }}>
                                      {step.expectedTools.join(', ')}
                                    </span>
                                  )}
                                  {step.result && (
                                    <span className="truncate max-w-xs text-[10px] px-1.5 py-0.5 rounded" style={{
                                      backgroundColor: step.status.toLowerCase() === 'failed' ? 'rgba(239, 68, 68, 0.1)' : 'var(--codex-bg)',
                                      color: step.status.toLowerCase() === 'failed' ? '#ef4444' : 'var(--codex-fg-subtle)',
                                    }} title={step.result}>
                                      {step.result}
                                    </span>
                                  )}
                                </div>
                              </div>

                              {/* Attempt count */}
                              <span className="text-[11px] mt-0.5" style={{
                                color: 'var(--codex-fg-subtle)',
                                fontFamily: 'var(--font-mono)',
                              }}>
                                {step.attemptCount}/{step.maxAttempts}
                              </span>
                            </motion.div>
                          ))}
                        </div>
                      )}
                    </motion.div>
                  )}
                </AnimatePresence>
              </motion.div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
