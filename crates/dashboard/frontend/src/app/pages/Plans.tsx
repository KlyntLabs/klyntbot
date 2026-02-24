import { useState } from 'react';
import { FileText, CheckCircle2, Circle, Clock, ChevronRight, Plus } from 'lucide-react';
import { motion } from 'motion/react';

type PlanStep = {
  id: string;
  title: string;
  status: 'completed' | 'active' | 'pending';
  duration: string;
};

type Plan = {
  id: string;
  title: string;
  description: string;
  status: 'active' | 'completed' | 'draft';
  progress: number;
  totalSteps: number;
  completedSteps: number;
  createdAt: string;
  steps: PlanStep[];
};

export default function Plans() {
  const [expandedPlan, setExpandedPlan] = useState<string | null>('1');

  const plans: Plan[] = [
    {
      id: '1',
      title: 'Authentication System Refactor',
      description: 'Complete overhaul of authentication module with improved security and testing',
      status: 'active',
      progress: 43,
      totalSteps: 7,
      completedSteps: 3,
      createdAt: '2 hours ago',
      steps: [
        { id: '1', title: 'Analyze current authentication flow', status: 'completed', duration: '15m' },
        { id: '2', title: 'Design new architecture', status: 'completed', duration: '30m' },
        { id: '3', title: 'Implement core auth functions', status: 'completed', duration: '45m' },
        { id: '4', title: 'Add error handling', status: 'active', duration: '20m' },
        { id: '5', title: 'Write unit tests', status: 'pending', duration: '40m' },
        { id: '6', title: 'Integration testing', status: 'pending', duration: '30m' },
        { id: '7', title: 'Documentation and review', status: 'pending', duration: '25m' }
      ]
    },
    {
      id: '2',
      title: 'Dashboard Performance Optimization',
      description: 'Optimize rendering and data fetching for improved user experience',
      status: 'draft',
      progress: 0,
      totalSteps: 5,
      completedSteps: 0,
      createdAt: '1 day ago',
      steps: [
        { id: '1', title: 'Profile current performance', status: 'pending', duration: '20m' },
        { id: '2', title: 'Identify bottlenecks', status: 'pending', duration: '30m' },
        { id: '3', title: 'Implement lazy loading', status: 'pending', duration: '45m' },
        { id: '4', title: 'Optimize database queries', status: 'pending', duration: '60m' },
        { id: '5', title: 'Verify improvements', status: 'pending', duration: '20m' }
      ]
    },
    {
      id: '3',
      title: 'API Documentation Update',
      description: 'Update all API endpoints with examples and error responses',
      status: 'completed',
      progress: 100,
      totalSteps: 4,
      completedSteps: 4,
      createdAt: '3 days ago',
      steps: [
        { id: '1', title: 'Audit existing documentation', status: 'completed', duration: '30m' },
        { id: '2', title: 'Update endpoint descriptions', status: 'completed', duration: '60m' },
        { id: '3', title: 'Add code examples', status: 'completed', duration: '45m' },
        { id: '4', title: 'Review and publish', status: 'completed', duration: '20m' }
      ]
    }
  ];

  const getStatusColor = (status: Plan['status']) => {
    switch (status) {
      case 'active': return 'var(--codex-accent)';
      case 'completed': return 'var(--codex-fg-subtle)';
      case 'draft': return 'var(--codex-fg-subtle)';
    }
  };

  const getStepStatusColor = (status: PlanStep['status']) => {
    switch (status) {
      case 'completed': return 'var(--codex-accent)';
      case 'active': return 'var(--codex-accent)';
      case 'pending': return 'var(--codex-fg-subtle)';
    }
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
              Plans
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
              New Plan
            </button>
          </div>

          {/* Stats */}
          <div className="flex gap-6 text-[13px]">
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-accent)' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>1 Active</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-fg-subtle)' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>1 Draft</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: 'var(--codex-fg-subtle)' }} />
              <span style={{ color: 'var(--codex-fg-muted)' }}>1 Completed</span>
            </div>
          </div>
        </div>
      </div>

      {/* Plans List */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-5xl mx-auto space-y-4">
          {plans.map((plan, index) => (
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
                onClick={() => setExpandedPlan(expandedPlan === plan.id ? null : plan.id)}
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
                        backgroundColor: plan.status === 'active' ? 'var(--codex-accent-dim)' : 'var(--codex-bg)',
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
                          Step {plan.completedSteps}/{plan.totalSteps}
                        </span>
                        <span style={{ color: 'var(--codex-fg-muted)' }}>{plan.progress}%</span>
                      </div>
                      <div className="h-1 rounded-full overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
                        <div
                          className="h-full rounded-full transition-all"
                          style={{
                            width: `${plan.progress}%`,
                            backgroundColor: 'var(--codex-accent)'
                          }}
                        />
                      </div>
                    </div>

                    <div className="flex items-center gap-2 mt-3 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                      <Clock className="w-3.5 h-3.5" strokeWidth={1.5} />
                      {plan.createdAt}
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
                  <div className="space-y-2">
                    {plan.steps.map((step, stepIndex) => (
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
                        {step.status === 'completed' ? (
                          <CheckCircle2 className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                        ) : step.status === 'active' ? (
                          <div className="w-4 h-4 rounded-full border-2 flex items-center justify-center" style={{
                            borderColor: getStepStatusColor(step.status)
                          }}>
                            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: getStepStatusColor(step.status) }} />
                          </div>
                        ) : (
                          <Circle className="w-4 h-4" strokeWidth={1.5} style={{ color: getStepStatusColor(step.status) }} />
                        )}

                        <span className="flex-1 text-[13px]" style={{
                          color: step.status === 'completed' ? 'var(--codex-fg-subtle)' : 'var(--codex-fg)',
                          textDecoration: step.status === 'completed' ? 'line-through' : 'none'
                        }}>
                          {step.title}
                        </span>

                        <span className="text-[11px]" style={{
                          color: 'var(--codex-fg-subtle)',
                          fontFamily: 'var(--font-mono)'
                        }}>
                          {step.duration}
                        </span>
                      </motion.div>
                    ))}
                  </div>
                </div>
              )}
            </motion.div>
          ))}
        </div>
      </div>
    </div>
  );
}
