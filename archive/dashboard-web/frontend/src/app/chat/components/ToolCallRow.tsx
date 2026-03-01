import { useState } from 'react';
import { Loader2, Check, X, ChevronRight, ChevronDown } from 'lucide-react';

export interface ToolCallRowData {
  name: string;
  args?: Record<string, unknown>;
  durationMs?: number;
  success?: boolean;
  completed?: boolean;
  result?: string;
}

function formatArgs(args?: Record<string, unknown>): string {
  if (!args || Object.keys(args).length === 0) return '{}';
  try {
    return JSON.stringify(args, null, 2);
  } catch {
    return String(args);
  }
}

export function ToolCallRow({ tc }: { tc: ToolCallRowData }) {
  const [expanded, setExpanded] = useState(false);
  const isCompleted = tc.completed ?? true;
  const hasDetails = tc.args || tc.result;
  const canExpand = hasDetails && isCompleted;

  const statusColor = isCompleted
    ? tc.success ? '#10b981' : '#ef4444'
    : 'var(--codex-accent)';

  return (
    <div
      className="rounded overflow-hidden"
      style={{ backgroundColor: 'rgba(0,0,0,0.2)' }}
    >
      <div
        className={`flex items-center gap-2 px-2 py-1.5 ${canExpand ? 'cursor-pointer hover:opacity-80' : ''}`}
        onClick={() => canExpand && setExpanded(!expanded)}
      >
        {isCompleted ? (
          tc.success ? (
            <Check className="w-3 h-3 flex-shrink-0" strokeWidth={2} style={{ color: '#10b981' }} />
          ) : (
            <X className="w-3 h-3 flex-shrink-0" strokeWidth={2} style={{ color: '#ef4444' }} />
          )
        ) : (
          <Loader2
            className="w-3 h-3 flex-shrink-0 animate-spin"
            strokeWidth={1.5}
            style={{ color: 'var(--codex-accent)' }}
          />
        )}

        <span
          className="text-[11px] truncate"
          style={{ color: statusColor, fontFamily: 'var(--font-mono)' }}
        >
          {tc.name}
        </span>

        {tc.durationMs != null && (
          <span
            className="text-[10px] ml-auto flex-shrink-0"
            style={{ color: '#666', fontFamily: 'var(--font-mono)' }}
          >
            {tc.durationMs}ms
          </span>
        )}

        {canExpand && (
          <span className="flex-shrink-0" style={{ color: '#555' }}>
            {expanded ? (
              <ChevronDown className="w-3 h-3" />
            ) : (
              <ChevronRight className="w-3 h-3" />
            )}
          </span>
        )}
      </div>

      {expanded && (
        <div
          className="px-2 pb-2 space-y-2 border-t"
          style={{ borderColor: 'rgba(255,255,255,0.06)' }}
        >
          {tc.args && Object.keys(tc.args).length > 0 && (
            <div className="pt-2">
              <div
                className="text-[10px] mb-1 uppercase tracking-wider"
                style={{ color: '#555', fontFamily: 'var(--font-mono)' }}
              >
                Args
              </div>
              <pre
                className="text-[10px] p-1.5 rounded overflow-x-auto whitespace-pre-wrap break-all"
                style={{
                  color: '#aaa',
                  backgroundColor: 'rgba(0,0,0,0.3)',
                  fontFamily: 'var(--font-mono)',
                  maxHeight: '120px',
                  overflowY: 'auto',
                }}
              >
                {formatArgs(tc.args)}
              </pre>
            </div>
          )}

          {tc.result && (
            <div>
              <div
                className="text-[10px] mb-1 uppercase tracking-wider"
                style={{ color: '#555', fontFamily: 'var(--font-mono)' }}
              >
                Result
              </div>
              <pre
                className="text-[10px] p-1.5 rounded overflow-x-auto whitespace-pre-wrap break-all"
                style={{
                  color: tc.success ? '#a3e635' : '#f87171',
                  backgroundColor: 'rgba(0,0,0,0.3)',
                  fontFamily: 'var(--font-mono)',
                  maxHeight: '200px',
                  overflowY: 'auto',
                }}
              >
                {tc.result}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
