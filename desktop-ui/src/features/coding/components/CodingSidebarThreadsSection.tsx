import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import Folder from "lucide-react/dist/esm/icons/folder";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { ChatThread } from "@/features/chat/types";
import type { WorkspaceInfo } from "@/types";
import {
  groupCodingThreadsByProject,
  type ProjectThreadGroup,
} from "../state/groupCodingThreadsByProject";
import { markThreadOpened } from "../state/ThreadEventBuffer";
import { TodoSidebarBadge } from "./TodoSidebarBadge";

type Props = {
  sessions: ChatThread[];
  runningIds: ReadonlySet<string>;
  recentlyCompleted: ReadonlyMap<string, number>;
  activeThreadId: string | null;
  onSelectThread: (sessionKey: string) => void;
  workspaces: ReadonlyArray<WorkspaceInfo>;
  workspaceIdByThread: ReadonlyMap<string, string>;
};

const COLLAPSED_KEY = "klynt:coding-sidebar:collapsed-projects";

function loadCollapsed(): Set<string> {
  if (typeof localStorage === "undefined") return new Set();
  try {
    const raw = localStorage.getItem(COLLAPSED_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    return Array.isArray(arr)
      ? new Set(arr.filter((x): x is string => typeof x === "string"))
      : new Set();
  } catch {
    return new Set();
  }
}

function statusFor(
  sessionKey: string,
  runningIds: ReadonlySet<string>,
  recent: ReadonlyMap<string, number>,
): "running" | "recent" | undefined {
  if (runningIds.has(sessionKey)) return "running";
  if (recent.has(sessionKey)) return "recent";
  return undefined;
}

/**
 * Coding-mode sidebar: project-grouped layout.
 *
 * Sessions are grouped by their workspace ("project"). Each project header is
 * collapsible; collapsed state persists in localStorage. Per-row dot indicates
 * running/recently-completed status.
 */
export function CodingSidebarThreadsSection({
  sessions,
  runningIds,
  recentlyCompleted,
  activeThreadId,
  onSelectThread,
  workspaces,
  workspaceIdByThread,
}: Props) {
  const groups = useMemo(
    () => groupCodingThreadsByProject(sessions, workspaceIdByThread, workspaces),
    [sessions, workspaceIdByThread, workspaces],
  );

  const [collapsed, setCollapsed] = useState<Set<string>>(() => loadCollapsed());

  useEffect(() => {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify(Array.from(collapsed)));
    } catch {
      // ignore quota / disabled storage
    }
  }, [collapsed]);

  const toggleCollapsed = useCallback((key: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const handleSelect = useCallback(
    (sessionKey: string) => {
      markThreadOpened(sessionKey);
      onSelectThread(sessionKey);
    },
    [onSelectThread],
  );

  if (groups.length === 0) {
    return (
      <>
        <div className="coding-sidebar-section-header coding-sidebar-section-header--top">
          <span>Projects</span>
        </div>
        <div className="coding-sidebar-empty">No projects yet</div>
      </>
    );
  }

  return (
    <>
      <div className="coding-sidebar-section-header coding-sidebar-section-header--top">
        <span>Projects</span>
      </div>
      {groups.map((group) => {
        const key = group.workspaceId ?? "__no_project__";
        const isCollapsed = collapsed.has(key);
        return (
          <ProjectGroup
            key={key}
            group={group}
            collapsed={isCollapsed}
            onToggle={() => toggleCollapsed(key)}
            runningIds={runningIds}
            recentlyCompleted={recentlyCompleted}
            activeThreadId={activeThreadId}
            onSelect={handleSelect}
          />
        );
      })}
    </>
  );
}

function ProjectGroup({
  group,
  collapsed,
  onToggle,
  runningIds,
  recentlyCompleted,
  activeThreadId,
  onSelect,
}: {
  group: ProjectThreadGroup;
  collapsed: boolean;
  onToggle: () => void;
  runningIds: ReadonlySet<string>;
  recentlyCompleted: ReadonlyMap<string, number>;
  activeThreadId: string | null;
  onSelect: (sessionKey: string) => void;
}) {
  const groupId = `coding-sidebar-project-${group.workspaceId ?? "no-project"}`;
  return (
    <div className="coding-sidebar-section coding-sidebar-project">
      <button
        type="button"
        className="coding-sidebar-project-header"
        aria-expanded={!collapsed}
        aria-controls={groupId}
        onClick={onToggle}
        title={group.path ?? group.name}
      >
        <span
          className={`coding-sidebar-project-chevron${collapsed ? "" : " is-open"}`}
          aria-hidden
        >
          <ChevronRight />
        </span>
        <span className="coding-sidebar-project-icon" aria-hidden>
          <Folder />
        </span>
        <span className="coding-sidebar-project-name">{group.name}</span>
        <span className="coding-sidebar-project-count">{group.threads.length}</span>
      </button>
      {!collapsed && (
        <div id={groupId} className="coding-sidebar-project-body">
          {group.threads.length === 0 ? (
            <div className="coding-sidebar-empty coding-sidebar-empty--nested">No chats</div>
          ) : (
            group.threads.map((t) => {
              const status = statusFor(t.sessionKey, runningIds, recentlyCompleted);
              const isActive = activeThreadId === t.sessionKey;
              return (
                <button
                  type="button"
                  key={t.sessionKey}
                  className={`thread-row coding-sidebar-thread${isActive ? " active" : ""}`}
                  data-status={status}
                  data-active={isActive ? "true" : undefined}
                  onClick={() => onSelect(t.sessionKey)}
                >
                  <span className="thread-status" aria-hidden />
                  <div className="thread-content">
                    <div className="thread-headline">
                      <span className="thread-name">{t.title}</span>
                    </div>
                    <TodoSidebarBadge threadId={t.sessionKey} />
                  </div>
                </button>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}
