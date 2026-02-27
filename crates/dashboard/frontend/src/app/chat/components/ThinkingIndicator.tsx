import { Loader2, Check, X } from 'lucide-react';
import type { ThinkingState } from '../../../lib/hooks/useAgent';
import { phaseLabel, strategyLabel } from '../utils';

export function ThinkingIndicator({ thinking }: { thinking: ThinkingState }) {
  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{
        backgroundColor: '#141414',
        border: '1px solid var(--codex-border)',
      }}
    >
      {/* Phase header */}
      <div
        className="px-3 py-2 flex items-center gap-2"
        style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderBottom: '1px solid var(--codex-border)',
        }}
      >
        <Loader2
          className="w-3.5 h-3.5 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
        <span
          className="text-[12px]"
          style={{
            color: 'var(--codex-fg)',
            fontFamily: 'var(--font-mono)',
          }}
        >
          {phaseLabel(thinking.phase)}
        </span>

        {/* Strategy badge */}
        {thinking.strategy && (
          <div
            className="flex items-center gap-1 px-2 py-0.5 rounded ml-auto"
            style={{
              backgroundColor: 'var(--codex-bg-tertiary)',
              border: '1px solid var(--codex-border)',
            }}
          >
            <span
              className="text-[10px]"
              style={{
                color: '#888',
                fontFamily: 'var(--font-mono)',
              }}
            >
              {strategyLabel(thinking.strategy)}
            </span>
            {thinking.confidence != null && (
              <>
                <span className="text-[10px]" style={{ color: '#666' }}>
                  &middot;
                </span>
                <span
                  className="text-[10px]"
                  style={{
                    color:
                      thinking.confidence > 0.8
                        ? 'var(--codex-accent)'
                        : thinking.confidence > 0.5
                          ? '#e5a00d'
                          : '#ef4444',
                    fontFamily: 'var(--font-mono)',
                    fontWeight: 500,
                  }}
                >
                  {Math.round(thinking.confidence * 100)}%
                </span>
              </>
            )}
          </div>
        )}

        {/* Iteration counter */}
        {thinking.iteration != null && thinking.maxIterations != null && (
          <span
            className="text-[10px] ml-2"
            style={{
              color: 'var(--codex-fg-subtle)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            {thinking.iteration}/{thinking.maxIterations}
          </span>
        )}
      </div>

      {/* Tool calls */}
      {thinking.toolCalls.length > 0 && (
        <div className="px-3 py-2 space-y-1.5">
          {thinking.toolCalls.map((tc, idx) => (
            <div
              key={`${tc.name}-${idx}`}
              className="flex items-center gap-2"
            >
              {tc.completed ? (
                tc.success ? (
                  <Check className="w-3 h-3" strokeWidth={2} style={{ color: '#10b981' }} />
                ) : (
                  <X className="w-3 h-3" strokeWidth={2} style={{ color: '#ef4444' }} />
                )
              ) : (
                <Loader2
                  className="w-3 h-3 animate-spin"
                  strokeWidth={1.5}
                  style={{ color: 'var(--codex-accent)' }}
                />
              )}
              <span
                className="px-1.5 py-0.5 rounded text-[10px] uppercase tracking-wide"
                style={{
                  backgroundColor: tc.completed
                    ? tc.success
                      ? 'rgba(16, 185, 129, 0.1)'
                      : 'rgba(239, 68, 68, 0.1)'
                    : 'var(--codex-accent-dim)',
                  color: tc.completed
                    ? tc.success
                      ? '#10b981'
                      : '#ef4444'
                    : 'var(--codex-accent)',
                  fontFamily: 'var(--font-mono)',
                  fontWeight: 500,
                }}
              >
                {tc.name}
              </span>
              {tc.durationMs != null && (
                <span
                  className="text-[10px]"
                  style={{
                    color: '#888',
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  {tc.durationMs}ms
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Pulsing dots when no tool calls yet */}
      {thinking.toolCalls.length === 0 && (
        <div className="px-3 py-3 flex gap-1.5">
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{ backgroundColor: 'var(--codex-accent)' }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{
              backgroundColor: 'var(--codex-accent)',
              animationDelay: '0.2s',
            }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full animate-pulse"
            style={{
              backgroundColor: 'var(--codex-accent)',
              animationDelay: '0.4s',
            }}
          />
        </div>
      )}
    </div>
  );
}
