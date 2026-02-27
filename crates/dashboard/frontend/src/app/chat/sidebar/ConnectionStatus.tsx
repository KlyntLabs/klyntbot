import { Loader2 } from 'lucide-react';
import type { ConnectionStatus as ConnectionStatusType } from '../../../lib/ws';
import { StatusDot } from '../components/MessageBubble';

interface ConnectionStatusBarProps {
  status: ConnectionStatusType;
  isStreaming: boolean;
}

export function ConnectionStatusBar({ status, isStreaming }: ConnectionStatusBarProps) {
  return (
    <div
      className="px-4 py-2.5 border-b flex items-center justify-between"
      style={{ borderColor: 'var(--codex-border-subtle)' }}
    >
      <StatusDot status={status} />
      {isStreaming && (
        <Loader2
          className="w-3 h-3 animate-spin"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
      )}
    </div>
  );
}
