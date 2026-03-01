import { useState } from 'react';
import { ChevronRight, ChevronDown, Wrench } from 'lucide-react';
import type { MessageToolCall } from '../../../lib/types';
import { ToolCallRow } from './ToolCallRow';

interface InlineToolCallsProps {
  toolCalls: MessageToolCall[];
}

export function InlineToolCalls({ toolCalls }: InlineToolCallsProps) {
  const [expanded, setExpanded] = useState(false);
  const failedCount = toolCalls.filter((tc) => !tc.success).length;
  const totalMs = toolCalls.reduce((sum, tc) => sum + (tc.durationMs ?? 0), 0);

  return (
    <div
      className="rounded-lg overflow-hidden mb-2"
      style={{
        backgroundColor: '#141414',
        border: '1px solid var(--codex-border)',
      }}
    >
      {/* Summary header — always visible */}
      <div
        className="flex items-center gap-2 px-3 py-2 cursor-pointer hover:opacity-80"
        onClick={() => setExpanded(!expanded)}
      >
        <Wrench
          className="w-3.5 h-3.5 flex-shrink-0"
          strokeWidth={1.5}
          style={{ color: '#666' }}
        />
        <span
          className="text-[11px]"
          style={{ color: '#888', fontFamily: 'var(--font-mono)' }}
        >
          {toolCalls.length} tool call{toolCalls.length !== 1 ? 's' : ''}
        </span>
        {failedCount > 0 && (
          <span
            className="text-[10px]"
            style={{ color: '#ef4444', fontFamily: 'var(--font-mono)' }}
          >
            ({failedCount} failed)
          </span>
        )}
        {totalMs > 0 && (
          <span
            className="text-[10px]"
            style={{ color: '#555', fontFamily: 'var(--font-mono)' }}
          >
            {totalMs}ms
          </span>
        )}
        <span className="ml-auto flex-shrink-0" style={{ color: '#555' }}>
          {expanded ? (
            <ChevronDown className="w-3.5 h-3.5" />
          ) : (
            <ChevronRight className="w-3.5 h-3.5" />
          )}
        </span>
      </div>

      {/* Expanded tool call list */}
      {expanded && (
        <div
          className="px-3 pb-2 space-y-1 border-t"
          style={{ borderColor: 'var(--codex-border)' }}
        >
          <div className="pt-2 space-y-1">
            {toolCalls.map((tc, idx) => (
              <ToolCallRow key={`${tc.name}-${idx}`} tc={tc} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
