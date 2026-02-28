import type { ConnectionStatus } from '../../../lib/ws';
import type { ToolActivityEntry } from '../../../lib/types';
import { ConnectionStatusBar } from './ConnectionStatus';
import { ToolActivityPanel } from './ToolActivityPanel';
import { QuickTasks } from './QuickTasks';
import { UpcomingEvents } from './UpcomingEvents';

interface ChatSidebarProps {
  status: ConnectionStatus;
  isStreaming: boolean;
  activeTools: Set<string>;
  toolHistory: ToolActivityEntry[];
}

export function ChatSidebar({
  status,
  isStreaming,
  activeTools,
  toolHistory,
}: ChatSidebarProps) {
  return (
    <aside
      className="w-[260px] border-l overflow-y-auto"
      style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)',
      }}
    >
      <ConnectionStatusBar status={status} isStreaming={isStreaming} />
      <ToolActivityPanel activeTools={activeTools} toolHistory={toolHistory} />
      <div className="border-b" style={{ borderColor: 'var(--codex-border-subtle)' }} />
      <QuickTasks />
      <UpcomingEvents />
    </aside>
  );
}
