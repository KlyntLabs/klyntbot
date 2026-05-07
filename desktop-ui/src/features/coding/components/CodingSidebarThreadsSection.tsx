import { useMemo } from "react";
import type { ChatThread } from "@/features/chat/types";
import { partitionCodingThreads } from "../state/partitionCodingThreads";
import { markThreadOpened } from "../state/ThreadEventBuffer";

type Props = {
  sessions: ChatThread[];
  runningIds: ReadonlySet<string>;
  recentlyCompleted: ReadonlyMap<string, number>;
  activeThreadId: string | null;
  onSelectThread: (sessionKey: string) => void;
};

/**
 * Coding-mode sidebar threads section. Renders three groups (Running /
 * Recently completed / Chats), collapsing any group whose count is zero.
 *
 * Uses a minimal local row component instead of the assistant-mode
 * `ThreadRow` because coding rows have no workspace, no subagent tree,
 * no pinning, and no `threadStatusById` pipeline — just title + dot +
 * active highlight.
 */
export function CodingSidebarThreadsSection({
  sessions,
  runningIds,
  recentlyCompleted,
  activeThreadId,
  onSelectThread,
}: Props) {
  const { running, recent, chats } = useMemo(
    () => partitionCodingThreads(sessions, runningIds, recentlyCompleted),
    [sessions, runningIds, recentlyCompleted],
  );

  const handleSelect = (sessionKey: string) => {
    markThreadOpened(sessionKey);
    onSelectThread(sessionKey);
  };

  return (
    <>
      {running.length > 0 && (
        <Section
          title="Running"
          count={running.length}
          rows={running}
          status="running"
          activeThreadId={activeThreadId}
          onSelect={handleSelect}
        />
      )}
      {recent.length > 0 && (
        <Section
          title="Recently completed"
          count={recent.length}
          rows={recent}
          status="recent"
          activeThreadId={activeThreadId}
          onSelect={handleSelect}
        />
      )}
      <Section
        title="Chats"
        count={chats.length}
        rows={chats}
        status={undefined}
        activeThreadId={activeThreadId}
        onSelect={handleSelect}
      />
    </>
  );
}

function Section({
  title,
  count,
  rows,
  status,
  activeThreadId,
  onSelect,
}: {
  title: string;
  count: number;
  rows: ChatThread[];
  status: "running" | "recent" | undefined;
  activeThreadId: string | null;
  onSelect: (sessionKey: string) => void;
}) {
  return (
    <div className="coding-sidebar-section">
      <div className="coding-sidebar-section-header">
        <span>{title}</span>
        <span className="coding-sidebar-section-count">{count}</span>
      </div>
      {rows.map((t) => (
        <button
          type="button"
          key={t.sessionKey}
          className={`thread-row${activeThreadId === t.sessionKey ? " active" : ""}`}
          data-status={status}
          data-active={activeThreadId === t.sessionKey ? "true" : undefined}
          onClick={() => onSelect(t.sessionKey)}
        >
          <span className="thread-status" aria-hidden />
          <div className="thread-content">
            <div className="thread-headline">
              <span className="thread-name">{t.title}</span>
            </div>
          </div>
        </button>
      ))}
    </div>
  );
}
