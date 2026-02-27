import { Loader2, Check, X } from 'lucide-react';
import type { ToolCallState, ThinkingState } from '../../../lib/hooks/useAgent';
import { SidebarSection } from './SidebarSection';

function ToolCallItem({ toolCall }: { toolCall: ToolCallState }) {
  return (
    <div
      className="flex items-center gap-2 p-1.5 rounded"
      style={{ backgroundColor: 'var(--codex-bg)' }}
    >
      {toolCall.completed ? (
        toolCall.success ? (
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
        style={{
          color: toolCall.completed
            ? toolCall.success ? '#10b981' : '#ef4444'
            : 'var(--codex-accent)',
          fontFamily: 'var(--font-mono)',
        }}
      >
        {toolCall.name}
      </span>
      {toolCall.durationMs != null && (
        <span
          className="text-[10px] ml-auto flex-shrink-0"
          style={{ color: '#888', fontFamily: 'var(--font-mono)' }}
        >
          {toolCall.durationMs}ms
        </span>
      )}
    </div>
  );
}

interface ToolCallListProps {
  thinking: ThinkingState | null;
}

export function ToolCallList({ thinking }: ToolCallListProps) {
  if (!thinking || thinking.toolCalls.length === 0) return null;

  return (
    <SidebarSection title="Tool Calls" open={true} onToggle={() => {}}>
      <div className="px-4 pb-4 space-y-2">
        {thinking.toolCalls.map((tc, idx) => (
          <ToolCallItem key={`${tc.name}-${idx}`} toolCall={tc} />
        ))}
      </div>
    </SidebarSection>
  );
}
