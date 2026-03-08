import { ChevronDown, FolderOpen } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import { useNavigate } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import { useSetToggle } from "../../hooks/useSetToggle";
import { formatRelativeTime } from "../../lib/dates";
import { sessionStatusBgColor, type TrackedSession } from "../../lib/session-types";

interface SessionSidebarProps {
  activeSessionId?: string;
}

function statusDot(status: TrackedSession["status"]) {
  return (
    <span
      className={`w-1.5 h-1.5 rounded-full shrink-0 ${sessionStatusBgColor(status)}`}
      title={status}
    />
  );
}

interface ProjectGroup {
  projectPath: string;
  projectName: string;
  sessions: TrackedSession[];
}

function groupByProject(sessions: TrackedSession[]): ProjectGroup[] {
  const map = new Map<string, ProjectGroup>();
  for (const s of sessions) {
    let group = map.get(s.projectPath);
    if (!group) {
      group = {
        projectPath: s.projectPath,
        projectName: s.projectName,
        sessions: [],
      };
      map.set(s.projectPath, group);
    }
    group.sessions.push(s);
  }
  return Array.from(map.values());
}

export function SessionSidebar({ activeSessionId }: SessionSidebarProps) {
  const navigate = useNavigate();
  const { data: sessions, refetch } = useQuery<TrackedSession[]>(
    "get_tracked_sessions",
    undefined,
    [],
  );

  useEvent<{ sessionId: string; status: string }>("session:status", refetch);
  useEvent<{ sessionId: string }>("session:new", refetch);

  const groups = useMemo(() => groupByProject(sessions), [sessions]);
  const [expandedGroups, toggleGroup] = useSetToggle();

  // Auto-expand all groups when sessions first load
  const hasExpandedRef = useRef(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: one-time expansion on first data load
  useEffect(() => {
    if (groups.length > 0 && !hasExpandedRef.current) {
      hasExpandedRef.current = true;
      for (const g of groups) {
        toggleGroup(`proj:${g.projectPath}`);
      }
    }
  }, [groups]);

  return (
    <div className="w-[260px] glass-sidebar flex flex-col">
      <div className="px-4 py-3">
        <span className="text-[13px] font-light text-secondary">Sessions</span>
      </div>

      <div className="mx-4 glass-divider" />

      <div className="flex-1 overflow-y-auto px-3 pb-3 pt-2">
        <div className="space-y-3">
          {groups.map((group) => {
            const groupKey = `proj:${group.projectPath}`;
            const isExpanded = expandedGroups.has(groupKey);
            return (
              <div key={group.projectPath}>
                <button
                  type="button"
                  onClick={() => toggleGroup(groupKey)}
                  aria-expanded={isExpanded}
                  className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.06] transition-colors text-[12px] font-light text-muted hover:text-secondary"
                >
                  <FolderOpen className="w-3.5 h-3.5" strokeWidth={1.5} />
                  <span className="flex-1 text-left truncate">{group.projectName}</span>
                  <span className="text-[10px] text-dim tabular-nums">{group.sessions.length}</span>
                  <ChevronDown
                    className={`w-3.5 h-3.5 transition-transform ${isExpanded ? "rotate-0" : "-rotate-90"}`}
                    strokeWidth={1.5}
                  />
                </button>
                {isExpanded && (
                  <div className="mt-1 ml-3 space-y-1">
                    {group.sessions.map((session) => {
                      const isActive = activeSessionId === session.sessionId;
                      return (
                        <button
                          type="button"
                          key={session.sessionId}
                          onClick={() => navigate(`/sessions/${session.sessionId}`)}
                          className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-lg transition-colors text-[12px] font-light ${
                            isActive
                              ? "bg-white/[0.08] text-primary"
                              : "text-muted hover:text-secondary hover:bg-white/[0.04]"
                          }`}
                        >
                          {statusDot(session.status)}
                          <span className="flex-1 text-left truncate">
                            {session.firstMessagePreview || session.gitBranch || "Session"}
                          </span>
                          {session.lastActivity && (
                            <span className="text-[11px] text-dim shrink-0 tabular-nums">
                              {formatRelativeTime(session.lastActivity)}
                            </span>
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}

          {sessions.length === 0 && (
            <div className="text-center py-8 text-muted text-[12px] font-light">
              No sessions tracked yet
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
