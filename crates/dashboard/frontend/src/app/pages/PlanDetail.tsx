import { useNavigate, useParams } from 'react-router';
import {
  ArrowLeft,
  CheckCircle2,
  Circle,
  Loader2,
  AlertTriangle,
  XCircle,
  SkipForward,
  Trash2,
  Play,
  Ban,
  Check,
  ChevronDown,
  ChevronRight,
  Clock,
} from 'lucide-react';
import { useState, useCallback, useEffect, useMemo } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { PlanWithSteps, PlanStep } from '../../lib/types';
import {
  formatTimeAgoLong,
  formatShortDate,
  getPlanStatusColor,
  isPlanActive,
} from '../../lib/utils';

// ── Helpers ──────────────────────────────────────────────────────────────────

function getStepStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'completed': return 'var(--codex-accent)';
    case 'executing': return 'var(--codex-accent)';
    case 'failed': return '#ef4444';
    case 'skipped': return 'var(--codex-fg-subtle)';
    default: return 'var(--codex-fg-subtle)';
  }
}

function computeProgress(steps: PlanStep[]): { completedSteps: number; totalSteps: number; progress: number } {
  const totalSteps = steps.length;
  const completedSteps = steps.filter((s) => s.status.toLowerCase() === 'completed').length;
  const progress = totalSteps > 0 ? Math.round((completedSteps / totalSteps) * 100) : 0;
  return { completedSteps, totalSteps, progress };
}

function getAvailableActions(status: string): string[] {
  switch (status.toLowerCase()) {
    case 'draft': return ['approve', 'abandon', 'delete'];
    case 'approved': return ['execute', 'abandon', 'delete'];
    case 'executing': return ['abandon'];
    case 'completed':
    case 'failed':
    case 'abandoned':
      return ['delete'];
    default: return [];
  }
}

function formatDuration(startIso: string, endIso: string): string {
  const ms = new Date(endIso).getTime() - new Date(startIso).getTime();
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  const hours = Math.floor(mins / 60);
  return `${hours}h ${mins % 60}m`;
}

function formatElapsed(createdAt: string, completedAt: string | null): string {
  const end = completedAt ? new Date(completedAt) : new Date();
  return formatDuration(createdAt, end.toISOString());
}


// ── Component ────────────────────────────────────────────────────────────────

export default function PlanDetail() {
  const { id } = useParams();
  const navigate = useNavigate();

  const { data, loading, error, setData } = useApi<PlanWithSteps>(`/api/plans/${id}`);
  const plan = data?.plan ?? null;
  const steps = data?.steps ?? [];

  // Editing
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleValue, setTitleValue] = useState('');
  const [descValue, setDescValue] = useState('');
  const [patchingField, setPatchingField] = useState<string | null>(null);
  const [editingLimit, setEditingLimit] = useState(false);
  const [limitValue, setLimitValue] = useState('');

  // Step result expansion
  const [expandedStep, setExpandedStep] = useState<string | null>(null);

  // Delete confirmation
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  // Status transition
  const [transitioning, setTransitioning] = useState(false);

  // Backtrack history collapse
  const [backtrackOpen, setBacktrackOpen] = useState(false);

  // Sync local editing state when plan loads
  useEffect(() => {
    if (plan) {
      setTitleValue(plan.title);
      setDescValue(plan.description);
      setLimitValue(String(plan.iterationLimit));
    }
  }, [plan]);

  // Polling when executing (3s)
  useEffect(() => {
    if (!plan || plan.status.toLowerCase() !== 'executing') return;
    const interval = setInterval(async () => {
      try {
        const fresh = await apiFetch<PlanWithSteps>(`/api/plans/${id}`);
        setData(fresh);
      } catch { /* ignore */ }
    }, 3000);
    return () => clearInterval(interval);
  }, [plan?.status, id, setData]);

  // ── Patch helper ──────────────────────────────────────────────────────────

  const patchPlan = useCallback(async (field: string, body: Record<string, unknown>) => {
    if (!id || !plan) return;
    setPatchingField(field);
    try {
      const updated = await apiFetch<PlanWithSteps>(`/api/plans/${encodeURIComponent(id)}`, {
        method: 'PATCH',
        body,
      });
      setData(updated);
    } catch { /* ignore */ } finally {
      setPatchingField(null);
    }
  }, [id, plan, setData]);

  // ── Title edit ──────────────────────────────────────────────────────────────

  const handleTitleBlur = useCallback(async () => {
    setEditingTitle(false);
    if (!plan || titleValue.trim() === plan.title) return;
    if (!titleValue.trim()) {
      setTitleValue(plan.title);
      return;
    }
    setData(prev => prev ? { ...prev, plan: { ...prev.plan, title: titleValue.trim() } } : prev);
    await patchPlan('title', { title: titleValue.trim() });
  }, [plan, titleValue, setData, patchPlan]);

  // ── Description edit ────────────────────────────────────────────────────────

  const handleDescBlur = useCallback(async () => {
    if (!plan) return;
    const newDesc = descValue.trim();
    if (newDesc === plan.description) return;
    setData(prev => prev ? { ...prev, plan: { ...prev.plan, description: newDesc } } : prev);
    await patchPlan('description', { description: newDesc });
  }, [plan, descValue, setData, patchPlan]);

  // ── Iteration limit edit ────────────────────────────────────────────────────

  const handleLimitBlur = useCallback(async () => {
    setEditingLimit(false);
    if (!plan) return;
    const newLimit = parseInt(limitValue) || plan.iterationLimit;
    if (newLimit === plan.iterationLimit) return;
    setData(prev => prev ? { ...prev, plan: { ...prev.plan, iterationLimit: newLimit } } : prev);
    await patchPlan('iterationLimit', { iterationLimit: newLimit });
  }, [plan, limitValue, setData, patchPlan]);

  // ── Status transition ──────────────────────────────────────────────────────

  const handleTransition = useCallback(async (action: string) => {
    if (!id || transitioning) return;
    setTransitioning(true);
    try {
      let newStatus: string;
      switch (action) {
        case 'approve': newStatus = 'approved'; break;
        case 'execute': newStatus = 'executing'; break;
        case 'abandon': newStatus = 'abandoned'; break;
        default: return;
      }
      const updated = await apiFetch<PlanWithSteps>(`/api/plans/${encodeURIComponent(id)}/status`, {
        body: { status: newStatus },
      });
      setData(updated);
    } catch { /* ignore */ } finally {
      setTransitioning(false);
    }
  }, [id, transitioning, setData]);

  // ── Delete ──────────────────────────────────────────────────────────────────

  const handleDelete = useCallback(async () => {
    if (!id) return;
    try {
      await apiFetch<void>(`/api/plans/${encodeURIComponent(id)}`, { method: 'DELETE' });
      navigate('/plans');
    } catch {
      setConfirmingDelete(false);
    }
  }, [id, navigate]);

  // ── Step icon ──────────────────────────────────────────────────────────────

  const getStepIcon = (step: PlanStep) => {
    const s = step.status.toLowerCase();
    switch (s) {
      case 'completed':
        return <CheckCircle2 className="w-5 h-5" strokeWidth={1.5} style={{ color: 'var(--codex-accent)' }} />;
      case 'executing':
        return <Loader2 className="w-5 h-5 animate-spin" strokeWidth={1.5} style={{ color: 'var(--codex-accent)' }} />;
      case 'failed':
        return <XCircle className="w-5 h-5" strokeWidth={1.5} style={{ color: '#ef4444' }} />;
      case 'skipped':
        return <SkipForward className="w-5 h-5" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />;
      default:
        return <Circle className="w-5 h-5" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />;
    }
  };

  // ── Computed values ────────────────────────────────────────────────────────

  const { completedSteps, totalSteps, progress } = useMemo(() => computeProgress(steps), [steps]);
  const actions = plan ? getAvailableActions(plan.status) : [];

  const stepCounts = useMemo(() => {
    const counts: Record<string, number> = { pending: 0, executing: 0, completed: 0, failed: 0, skipped: 0 };
    for (const step of steps) {
      const s = step.status.toLowerCase();
      if (s in counts) counts[s]++;
      else counts['pending']++;
    }
    return counts;
  }, [steps]);

  const backtrackHistory = useMemo(() => {
    if (!plan?.backtrackHistory) return [];
    if (Array.isArray(plan.backtrackHistory)) return plan.backtrackHistory as unknown[];
    return [];
  }, [plan?.backtrackHistory]);

  // ── Loading / Error / Not found ─────────────────────────────────────────

  if (loading && !plan) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <Loader2 className="w-5 h-5 animate-spin" style={{ color: 'var(--codex-fg-subtle)' }} />
      </div>
    );
  }

  if (error && !plan) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2" style={{ backgroundColor: 'var(--codex-bg)', color: 'var(--codex-fg-subtle)' }}>
        <AlertTriangle className="w-4 h-4" strokeWidth={1.5} />
        <span className="text-[13px]">Failed to load plan</span>
      </div>
    );
  }

  if (!plan) return null;

  const statusLower = plan.status.toLowerCase();

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
              onClick={() => navigate('/plans')}
              className="flex items-center gap-2 text-[13px] transition-colors"
              style={{ color: 'var(--codex-fg-subtle)' }}
              onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg)'}
              onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
            >
              <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
              Plans
            </button>

            <div className="text-[13px]" style={{
              color: 'var(--codex-accent)',
              fontFamily: 'var(--font-mono)'
            }}>
              {plan.id.slice(0, 8)}
            </div>

            {/* Status badge */}
            <span className="px-3 py-1.5 rounded text-[12px]" style={{
              backgroundColor: getPlanStatusColor(plan.status) + '20',
              color: getPlanStatusColor(plan.status),
              border: `1px solid ${getPlanStatusColor(plan.status)}40`
            }}>
              {plan.status}
            </span>

            {/* Action buttons */}
            {actions.includes('approve') && (
              <button
                onClick={() => handleTransition('approve')}
                disabled={transitioning}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] transition-colors"
                style={{
                  color: 'var(--codex-accent)',
                  border: '1px solid var(--codex-accent)',
                  opacity: transitioning ? 0.5 : 1
                }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'rgba(16, 163, 127, 0.1)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
              >
                <Check className="w-3.5 h-3.5" strokeWidth={1.5} />
                Approve
              </button>
            )}

            {actions.includes('execute') && (
              <button
                onClick={() => handleTransition('execute')}
                disabled={transitioning}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] transition-colors"
                style={{
                  backgroundColor: 'var(--codex-accent)',
                  color: 'white',
                  opacity: transitioning ? 0.5 : 1
                }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
              >
                {transitioning ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Play className="w-3.5 h-3.5" strokeWidth={1.5} />}
                Execute
              </button>
            )}

            {actions.includes('abandon') && (
              <button
                onClick={() => handleTransition('abandon')}
                disabled={transitioning}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] transition-colors"
                style={{
                  color: 'var(--codex-fg-subtle)',
                  border: '1px solid var(--codex-border)',
                  opacity: transitioning ? 0.5 : 1
                }}
                onMouseEnter={(e) => { e.currentTarget.style.color = '#ef4444'; e.currentTarget.style.borderColor = '#ef4444'; }}
                onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--codex-fg-subtle)'; e.currentTarget.style.borderColor = 'var(--codex-border)'; }}
              >
                <Ban className="w-3.5 h-3.5" strokeWidth={1.5} />
                Abandon
              </button>
            )}

            <div className="flex-1" />

            {/* Delete button */}
            {actions.includes('delete') && (
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
            )}
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
                  Delete <strong style={{ color: 'var(--codex-fg)' }}>{plan.title}</strong>? This will also delete all steps. This cannot be undone.
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

        {/* Content Area */}
        <div className="flex-1 overflow-y-auto p-8" style={{ backgroundColor: 'var(--codex-bg)' }}>
          <div className="max-w-4xl mx-auto">
            {/* Title -- editable input */}
            {editingTitle ? (
              <input
                type="text"
                value={titleValue}
                onChange={(e) => setTitleValue(e.target.value)}
                onBlur={handleTitleBlur}
                onKeyDown={(e) => { if (e.key === 'Enter') e.currentTarget.blur(); if (e.key === 'Escape') { setTitleValue(plan.title); setEditingTitle(false); } }}
                autoFocus
                className="w-full text-2xl font-bold mb-6 bg-transparent border-none outline-none"
                style={{ color: 'var(--codex-fg)' }}
                placeholder="Plan title..."
              />
            ) : (
              <h1
                className="text-2xl font-bold mb-6 cursor-pointer"
                style={{ color: 'var(--codex-fg)' }}
                onClick={() => { if (statusLower === 'draft' || statusLower === 'approved') setEditingTitle(true); }}
                title={statusLower === 'draft' || statusLower === 'approved' ? 'Click to edit' : undefined}
              >
                {plan.title}
              </h1>
            )}

            {/* Description -- editable textarea */}
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
                  readOnly={statusLower !== 'draft' && statusLower !== 'approved'}
                  className="w-full p-4 min-h-[120px] bg-transparent outline-none resize-none"
                  style={{
                    color: 'var(--codex-fg)',
                    fontSize: '14px',
                    lineHeight: '1.6',
                    cursor: (statusLower === 'draft' || statusLower === 'approved') ? 'text' : 'default',
                  }}
                />
              </div>
            </div>

            {/* Step Timeline */}
            <div className="mb-8">
              <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                Steps ({completedSteps}/{totalSteps})
              </h3>

              {steps.length === 0 && (
                <div className="text-[13px] py-8 text-center" style={{ color: 'var(--codex-fg-muted)' }}>
                  No steps defined
                </div>
              )}

              <div className="relative">
                {steps.map((step, idx) => {
                  const isLast = idx === steps.length - 1;
                  const isExpanded = expandedStep === step.id;
                  const stepStatusLower = step.status.toLowerCase();
                  const isFailed = stepStatusLower === 'failed';

                  return (
                    <div key={step.id} className="relative flex gap-4" style={{ paddingBottom: isLast ? 0 : 24 }}>
                      {/* Vertical connector line */}
                      {!isLast && (
                        <div
                          className="absolute"
                          style={{
                            left: 10,
                            top: 28,
                            bottom: 0,
                            width: 2,
                            backgroundColor: getStepStatusColor(step.status) + '40',
                          }}
                        />
                      )}

                      {/* Status icon */}
                      <div className="flex-shrink-0 relative z-10" style={{ width: 22 }}>
                        {getStepIcon(step)}
                      </div>

                      {/* Step content */}
                      <div className="flex-1 min-w-0">
                        <div
                          className="p-3 rounded-lg border cursor-pointer transition-colors"
                          style={{
                            backgroundColor: isExpanded ? '#1a1a1a' : '#141414',
                            borderColor: stepStatusLower === 'executing' ? 'var(--codex-accent)' : 'var(--codex-border)',
                          }}
                          onClick={() => setExpandedStep(isExpanded ? null : step.id)}
                          onMouseEnter={(e) => { if (!isExpanded) e.currentTarget.style.backgroundColor = '#1a1a1a'; }}
                          onMouseLeave={(e) => { if (!isExpanded) e.currentTarget.style.backgroundColor = '#141414'; }}
                        >
                          {/* Step header */}
                          <div className="flex items-start gap-2 mb-1">
                            <span className="text-[11px] px-1.5 py-0.5 rounded flex-shrink-0" style={{
                              backgroundColor: getStepStatusColor(step.status) + '20',
                              color: getStepStatusColor(step.status),
                              fontFamily: 'var(--font-mono)',
                            }}>
                              #{step.stepIndex}
                            </span>
                            <span className="text-[13px] flex-1" style={{ color: 'var(--codex-fg)' }}>
                              {step.description}
                            </span>
                            <ChevronRight
                              className="w-4 h-4 flex-shrink-0 transition-transform"
                              strokeWidth={1.5}
                              style={{
                                color: 'var(--codex-fg-subtle)',
                                transform: isExpanded ? 'rotate(90deg)' : 'rotate(0deg)',
                              }}
                            />
                          </div>

                          {/* Step metadata row */}
                          <div className="flex items-center gap-3 mt-2 flex-wrap">
                            {/* Duration badge */}
                            {step.startedAt && step.completedAt && (
                              <span className="flex items-center gap-1 text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                                <Clock className="w-3 h-3" strokeWidth={1.5} />
                                {formatDuration(step.startedAt, step.completedAt)}
                              </span>
                            )}

                            {/* Attempt count */}
                            <span className="text-[11px]" style={{ color: 'var(--codex-fg-muted)' }}>
                              {step.attemptCount}/{step.maxAttempts} attempts
                            </span>

                            {/* Expected tools */}
                            {step.expectedTools.length > 0 && (
                              <div className="flex items-center gap-1 flex-wrap">
                                {step.expectedTools.map((tool) => (
                                  <span
                                    key={tool}
                                    className="px-1.5 py-0.5 rounded text-[10px]"
                                    style={{
                                      backgroundColor: 'var(--codex-bg)',
                                      color: 'var(--codex-fg-subtle)',
                                      border: '1px solid var(--codex-border)',
                                      fontFamily: 'var(--font-mono)',
                                    }}
                                  >
                                    {tool}
                                  </span>
                                ))}
                              </div>
                            )}
                          </div>

                          {/* Expanded result */}
                          <AnimatePresence>
                            {isExpanded && step.result && (
                              <motion.div
                                initial={{ opacity: 0, height: 0 }}
                                animate={{ opacity: 1, height: 'auto' }}
                                exit={{ opacity: 0, height: 0 }}
                                className="mt-3 pt-3 border-t"
                                style={{ borderColor: 'var(--codex-border)' }}
                              >
                                {/* Reasoning */}
                                {step.reasoning && (
                                  <div className="mb-3">
                                    <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Reasoning</div>
                                    <div className="text-[12px]" style={{ color: 'var(--codex-fg-muted)', lineHeight: '1.5' }}>
                                      {step.reasoning}
                                    </div>
                                  </div>
                                )}

                                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Result</div>
                                <div
                                  className="p-3 rounded text-[12px] whitespace-pre-wrap break-words"
                                  style={{
                                    backgroundColor: isFailed ? 'rgba(239, 68, 68, 0.1)' : 'var(--codex-bg)',
                                    color: isFailed ? '#ef4444' : 'var(--codex-fg)',
                                    border: `1px solid ${isFailed ? 'rgba(239, 68, 68, 0.3)' : 'var(--codex-border)'}`,
                                    fontFamily: 'var(--font-mono)',
                                    fontSize: '12px',
                                    lineHeight: '1.5',
                                    maxHeight: '300px',
                                    overflowY: 'auto',
                                  }}
                                >
                                  {step.result}
                                </div>
                              </motion.div>
                            )}
                          </AnimatePresence>

                          {/* Show reasoning inline if expanded but no result */}
                          <AnimatePresence>
                            {isExpanded && !step.result && step.reasoning && (
                              <motion.div
                                initial={{ opacity: 0, height: 0 }}
                                animate={{ opacity: 1, height: 'auto' }}
                                exit={{ opacity: 0, height: 0 }}
                                className="mt-3 pt-3 border-t"
                                style={{ borderColor: 'var(--codex-border)' }}
                              >
                                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Reasoning</div>
                                <div className="text-[12px]" style={{ color: 'var(--codex-fg-muted)', lineHeight: '1.5' }}>
                                  {step.reasoning}
                                </div>
                              </motion.div>
                            )}
                          </AnimatePresence>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            {/* Backtrack History */}
            {backtrackHistory.length > 0 && (
              <div className="mb-8">
                <button
                  onClick={() => setBacktrackOpen(!backtrackOpen)}
                  className="flex items-center gap-2 text-[13px] mb-3 transition-colors"
                  style={{ color: 'var(--codex-fg-subtle)' }}
                  onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg)'}
                  onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                >
                  {backtrackOpen ? (
                    <ChevronDown className="w-4 h-4" strokeWidth={1.5} />
                  ) : (
                    <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
                  )}
                  Backtrack History ({backtrackHistory.length})
                </button>

                <AnimatePresence>
                  {backtrackOpen && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      className="space-y-2"
                    >
                      {backtrackHistory.map((entry, idx) => {
                        const entryObj = entry as Record<string, unknown>;
                        return (
                          <div
                            key={idx}
                            className="p-3 rounded-lg border text-[12px]"
                            style={{
                              backgroundColor: '#141414',
                              borderColor: 'var(--codex-border)',
                            }}
                          >
                            <div className="flex items-center gap-2 mb-1">
                              <span className="text-[11px] px-1.5 py-0.5 rounded" style={{
                                backgroundColor: 'rgba(239, 68, 68, 0.1)',
                                color: '#ef4444',
                                fontFamily: 'var(--font-mono)',
                              }}>
                                Backtrack #{idx + 1}
                              </span>
                              {entryObj.stepIndex != null && (
                                <span style={{ color: 'var(--codex-fg-muted)' }}>
                                  at step {String(entryObj.stepIndex)}
                                </span>
                              )}
                            </div>
                            {entryObj.reason && (
                              <div style={{ color: 'var(--codex-fg-subtle)', lineHeight: '1.5' }}>
                                {String(entryObj.reason)}
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Right Panel */}
      <aside className="w-[420px] border-l overflow-y-auto shrink-0" style={{
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
                <span className="px-2 py-0.5 rounded text-[11px]" style={{
                  backgroundColor: getPlanStatusColor(plan.status) + '20',
                  color: getPlanStatusColor(plan.status),
                }}>
                  {plan.status}
                </span>
              </div>

              {/* Iteration Limit */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Iteration Limit</span>
                {editingLimit ? (
                  <input
                    type="number"
                    min="1"
                    value={limitValue}
                    onChange={(e) => setLimitValue(e.target.value)}
                    onBlur={handleLimitBlur}
                    onKeyDown={(e) => { if (e.key === 'Enter') e.currentTarget.blur(); if (e.key === 'Escape') { setLimitValue(String(plan.iterationLimit)); setEditingLimit(false); } }}
                    autoFocus
                    className="w-16 px-2 py-1 rounded text-[11px] outline-none text-right"
                    style={{
                      backgroundColor: 'var(--codex-bg)',
                      color: 'var(--codex-fg)',
                      border: '1px solid var(--codex-accent)',
                    }}
                  />
                ) : (
                  <span
                    className="px-2 py-1 rounded text-[11px] cursor-pointer transition-colors"
                    style={{
                      color: 'var(--codex-fg)',
                      border: '1px solid var(--codex-border)',
                      backgroundColor: 'var(--codex-bg)',
                    }}
                    onClick={() => {
                      if (statusLower === 'draft' || statusLower === 'approved') {
                        setEditingLimit(true);
                      }
                    }}
                    title={statusLower === 'draft' || statusLower === 'approved' ? 'Click to edit' : undefined}
                  >
                    {plan.iterationLimit}
                  </span>
                )}
              </div>

              {/* Current Step */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Current Step</span>
                <span style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                  {plan.currentStepIndex} / {totalSteps}
                </span>
              </div>

              {/* Visibility */}
              {plan.visibility && plan.visibility !== 'transparent' && (
                <div className="flex justify-between items-center text-[12px]">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>Visibility</span>
                  <span className="px-2 py-0.5 rounded text-[10px] tracking-wide" style={{
                    backgroundColor: 'var(--codex-bg)',
                    color: plan.visibility === 'silent' ? '#6b7280' : '#f59e0b',
                    border: '1px solid var(--codex-border)',
                  }}>
                    {plan.visibility === 'silent' ? 'silent' : 'on failure'}
                  </span>
                </div>
              )}

              {/* Task ID */}
              {plan.taskId && (
                <div className="flex justify-between items-center text-[12px]">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>Linked Task</span>
                  <span className="text-[11px] truncate max-w-[200px]" style={{
                    color: 'var(--codex-accent)',
                    fontFamily: 'var(--font-mono)',
                    cursor: 'pointer',
                  }}
                    onClick={() => navigate(`/tasks/${plan.taskId}`)}
                  >
                    {plan.taskId.slice(0, 8)}...
                  </span>
                </div>
              )}

              {/* Session Key */}
              <div className="flex justify-between items-center text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Session</span>
                <span className="text-[11px] truncate max-w-[200px]" style={{
                  color: 'var(--codex-fg-muted)',
                  fontFamily: 'var(--font-mono)',
                }}>
                  {plan.sessionKey}
                </span>
              </div>

              {/* Goal ID */}
              {plan.goalId && (
                <div className="flex justify-between items-center text-[12px]">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>Goal</span>
                  <span className="text-[11px] truncate max-w-[200px]" style={{
                    color: 'var(--codex-fg-muted)',
                    fontFamily: 'var(--font-mono)',
                  }}>
                    {plan.goalId.slice(0, 8)}
                  </span>
                </div>
              )}

              <div className="flex justify-between text-[12px] pt-2 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Created</span>
                <span style={{ color: 'var(--codex-fg)' }}>{formatShortDate(plan.createdAt)}</span>
              </div>

              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Updated</span>
                <span style={{ color: 'var(--codex-fg)' }}>{formatTimeAgoLong(plan.updatedAt)}</span>
              </div>

              {plan.completedAt && (
                <div className="flex justify-between text-[12px]">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>Completed</span>
                  <span style={{ color: 'var(--codex-fg)' }}>{formatShortDate(plan.completedAt)}</span>
                </div>
              )}
            </div>
          </div>

          {/* Progress Card */}
          <div className="p-4 rounded-lg" style={{
            backgroundColor: '#141414',
            border: '1px solid var(--codex-border)'
          }}>
            <h3 className="text-[13px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
              Progress
            </h3>
            <div className="space-y-4">
              {/* Progress bar */}
              <div>
                <div className="flex justify-between text-[12px] mb-2">
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>
                    {completedSteps} of {totalSteps} steps
                  </span>
                  <span style={{ color: isPlanActive(plan.status) ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)' }}>
                    {progress}%
                  </span>
                </div>
                <div className="h-2 rounded-full overflow-hidden" style={{
                  backgroundColor: 'var(--codex-bg)',
                }}>
                  <motion.div
                    className="h-full rounded-full"
                    style={{
                      backgroundColor: statusLower === 'failed' ? '#ef4444' : 'var(--codex-accent)',
                    }}
                    initial={{ width: 0 }}
                    animate={{ width: `${progress}%` }}
                    transition={{ duration: 0.5, ease: 'easeOut' }}
                  />
                </div>
              </div>

              {/* Elapsed time */}
              <div className="flex justify-between text-[12px]">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Elapsed</span>
                <span style={{
                  color: isPlanActive(plan.status) ? 'var(--codex-accent)' : 'var(--codex-fg)',
                  fontFamily: 'var(--font-mono)',
                }}>
                  {formatElapsed(plan.createdAt, plan.completedAt)}
                </span>
              </div>

              {/* Step counts by status */}
              <div className="pt-3 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                <div className="grid grid-cols-2 gap-2">
                  {Object.entries(stepCounts).map(([status, count]) => (
                    <div key={status} className="flex items-center gap-2 text-[11px]">
                      <div className="w-2 h-2 rounded-full" style={{
                        backgroundColor: status === 'completed' ? 'var(--codex-accent)'
                          : status === 'executing' ? 'var(--codex-accent)'
                          : status === 'failed' ? '#ef4444'
                          : status === 'skipped' ? 'var(--codex-fg-subtle)'
                          : 'var(--codex-fg-muted)',
                      }} />
                      <span style={{ color: 'var(--codex-fg-subtle)' }}>
                        {status.charAt(0).toUpperCase() + status.slice(1)}
                      </span>
                      <span style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                        {count}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </aside>
    </div>
  );
}
