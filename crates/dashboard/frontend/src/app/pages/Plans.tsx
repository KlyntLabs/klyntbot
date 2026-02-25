import { useState, useCallback } from 'react';
import { CheckCircle2, Circle, Clock, ChevronRight, Plus, Loader2, AlertTriangle, XCircle, SkipForward } from 'lucide-react';
import { motion } from 'motion/react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { Plan, PlanWithSteps, PlanStep } from '../../lib/types';

/** Format an ISO date string as a relative time description. */
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

/**
 * Map backend plan status to a badge color.
 * 'Executing' and 'Approved' are active (accent), everything else is subtle.
 */
function getStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'executing':
    case 'approved':
      return 'var(--codex-accent)';
    default:
      return 'var(--codex-fg-subtle)';
  }
}

/** Is this plan status considered "active" for the accent-dim badge background? */
function isActiveStatus(status: string): boolean {
  const s = status.toLowerCase();
  return s === 'executing' || s === 'approved';
}

/**
 * Map backend step status to a display color.
 * 'Completed' and 'InProgress' are accent-colored, everything else is subtle.
 */
function getStepStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'completed':
    case 'in_progress':
      return 'var(--codex-accent)';
    default:
      return 'var(--codex-fg-subtle)';
  }
}

/** Compute progress percentage from steps. */
function computeProgress(steps: PlanStep[]): { completedSteps: number; totalSteps: number; progress: number } {
  const totalSteps = steps.length;
  const completedSteps = steps.filter((s) => s.status.toLowerCase() === 'completed').length;
  const progress = totalSteps > 0 ? Math.round((completedSteps / totalSteps) * 100) : 0;
  return { completedSteps, totalSteps, progress };
}

export default function Plans() {
  const [expandedPlan, setExpandedPlan] = useState<string | null>(null);
  const [expandedPlanSteps, setExpandedPlanSteps] = useState<Record<string, PlanStep[]>>({});
  const [stepsLoading, setStepsLoading] = useState<Record<string, boolean>>({});

  const { data: plans, loading, error } = useApi<Plan[]>('/api/plans');
  const planList = plans ?? [];

  const handleTogglePlan = useCallback(
    async (planId: string) => {
      if (expandedPlan === planId) {
        setExpandedPlan(null);
        return;
      }

      setExpandedPlan(planId);

      // Fetch steps if we don't already have them for this plan
      if (!expandedPlanSteps[planId]) {
        setStepsLoading((prev) => ({ ...prev, [planId]: true }));
        try {
          const result = await apiFetch<PlanWithSteps>('/api/plans/' + planId);
          setExpandedPlanSteps((prev) => ({ ...prev, [planId]: result.steps }));
        } catch {
          // On error, store an empty array so we don't keep retrying
          setExpandedPlanSteps((prev) => ({ ...prev, [planId]: [] }));
        } finally {
          setStepsLoading((prev) => ({ ...prev, [planId]: false }));
        }
      }
    },
    [expandedPlan, expandedPlanSteps],
  );

  // Dynamic stats computed from live data
  const activeCount = planList.filter((p) => { const s = p.status.toLowerCase(); return s === 'executing' || s === 'approved'; }).length;
  const draftCount = planList.filter((p) => p.status.toLowerCase() === 'draft').length;
  const completedCount = planList.filter((p) => p.status.toLowerCase() === 'completed').length;

  // Loading state
  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <Loader2
          className="w-6 h-6 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-fg-subtle)' }}
        />
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <AlertTriangle className="w-6 h-6" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
        <span className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
          Failed to load plans
        </span>
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
              Plans
            </h1>

            {/* TODO: Implement plan creation */}
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
              New Plan
            </button>
          </div>

          {/* Stats */}
          <div className="flex gap-6 text-[13px]">
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-accent)' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>{activeCount} Active</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-fg-subtle)' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>{draftCount} Draft</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-fg-subtle)' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>{completedCount} Completed</span>
            </div>
          </div>
        </div>
      </div>

      {/* Plans List */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-5xl mx-auto space-y-4">
          {planList.length === 0 && (
            <div className="flex flex-col items-center justify-center py-16 gap-3">
              <span className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                No plans yet
              </span>
            </div>
          )}
          {planList.map((plan, index) => {
            const steps = expandedPlanSteps[plan.id] ?? [];
            const { completedSteps, totalSteps, progress } = steps.length > 0
              ? computeProgress(steps)
              : { completedSteps: plan.currentStepIndex, totalSteps: 0, progress: 0 };

            return (
              <motion.div
                key={plan.id}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: index * 0.05 }}
                className="rounded-lg border"
                style={{
                  backgroundColor: 'var(--codex-bg-tertiary)',
                  borderColor: 'var(--codex-border)'
                }}
              >
                {/* Plan Header */}
                <button
                  onClick={() => handleTogglePlan(plan.id)}
                  className="w-full p-5 text-left transition-colors"
                  onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'}
                  onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                >
                  <div className="flex items-start gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-3 mb-2">
                        <h3 className="text-[15px]" style={{
                          color: 'var(--codex-fg)',
                          fontWeight: 400
                        }}>
                          {plan.title}
                        </h3>
                        <span className="px-2 py-0.5 rounded text-[10px] uppercase tracking-wide" style={{
                          backgroundColor: isActiveStatus(plan.status) ? 'var(--codex-accent-dim)' : 'var(--codex-bg)',
                          color: getStatusColor(plan.status),
                          border: '1px solid var(--codex-border)'
                        }}>
                          {plan.status}
                        </span>
                      </div>

                      <p className="text-[13px] mb-4" style={{ color: 'var(--codex-fg-subtle)' }}>
                        {plan.description}
                      </p>

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
                            style={{
                              width: `${progress}%`,
                              backgroundColor: 'var(--codex-accent)'
                            }}
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
                        transform: expandedPlan === plan.id ? 'rotate(90deg)' : 'rotate(0deg)'
                      }}
                    />
                  </div>
                </button>

                {/* Plan Steps */}
                {expandedPlan === plan.id && (
                  <div className="border-t px-5 py-4" style={{ borderColor: 'var(--codex-border)' }}>
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
                      <div className="space-y-2">
                        {steps.map((step, stepIndex) => (
                          <motion.div
                            key={step.id}
                            initial={{ opacity: 0, x: -10 }}
                            animate={{ opacity: 1, x: 0 }}
                            transition={{ delay: stepIndex * 0.05 }}
                            className="flex items-center gap-3 p-3 rounded-lg transition-colors"
                            style={{ backgroundColor: 'transparent' }}
                            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'}
                            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                          >
                            {step.status.toLowerCase() === 'completed' ? (
                              <CheckCircle2 className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                            ) : step.status.toLowerCase() === 'in_progress' ? (
                              <div className="w-4 h-4 rounded-full border-2 flex items-center justify-center" style={{
                                borderColor: getStepStatusColor(step.status)
                              }}>
                                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getStepStatusColor(step.status) }} />
                              </div>
                            ) : step.status.toLowerCase() === 'failed' ? (
                              <XCircle className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                            ) : step.status.toLowerCase() === 'skipped' ? (
                              <SkipForward className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                            ) : (
                              <Circle className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                            )}

                            <span className="flex-1 text-[13px]" style={{
                              color: step.status.toLowerCase() === 'completed' ? 'var(--codex-fg-subtle)' : 'var(--codex-fg)',
                              textDecoration: step.status.toLowerCase() === 'completed' ? 'line-through' : 'none'
                            }}>
                              {step.description}
                            </span>

                            <span className="text-[11px]" style={{
                              color: 'var(--codex-fg-subtle)',
                              fontFamily: 'var(--font-mono)'
                            }}>
                              {step.attemptCount}/{step.maxAttempts}
                            </span>
                          </motion.div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </motion.div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
