import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Send } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import { useSetToggle } from "../../hooks/useSetToggle";
import { useTransparentBackground } from "../../hooks/useTransparentBackground";
import type { AgentStatus as AgentStatusType, CalendarEvent, TodayTask } from "../../lib/types";
import { isTauri } from "../../lib/utils";
import { Badge } from "../ui/Badge";
import { Checkbox } from "../ui/Checkbox";
import { KlyntLogo } from "../ui/KlyntLogo";

type DisplayStatus = "active" | "idle";

/** Color of the left border indicator based on task state */
function taskIndicatorClass(task: TodayTask, isCompleted: boolean): string {
  if (isCompleted) return "border-l-dim";
  if (task.isOverdue) return "border-l-destructive";
  if (task.isDueToday) return "border-l-brand";
  if (task.status === "doing") return "border-l-info";
  return "border-l-transparent";
}

export function SystemTray() {
  const { data: agentStatusData } = useQuery<AgentStatusType>("agent_status", undefined, {
    status: "active",
    activeTaskCount: 3,
    focusTask: null,
  });
  const { data: todayTasks, refetch: refetchTasks } = useQuery<TodayTask[]>(
    "today_tasks",
    undefined,
    [],
  );
  const { data: calendarEvents } = useQuery<CalendarEvent[]>("calendar_events", { limit: 5 }, []);

  const toggleComplete = useMutation<TodayTask, { id: string }>("task_toggle_complete");
  const [chatInput, setChatInput] = useState("");
  const [completedIds, toggleCompletedId] = useSetToggle();

  // Auto-refresh when entities change
  useEvent<{ entityKind: string; id: string }>("entity:updated", () => {
    refetchTasks();
  });

  const displayStatus: DisplayStatus = agentStatusData.status === "active" ? "active" : "idle";

  const statusColors: Record<DisplayStatus, string> = {
    active: "var(--color-success)",
    idle: "var(--color-brand)",
  };

  const handleToggleTask = async (taskId: string) => {
    toggleCompletedId(taskId);
    await toggleComplete.mutate({ id: taskId });
  };

  const handleChatSubmit = async () => {
    const text = chatInput.trim();
    if (!text) return;

    if (isTauri) {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const { emit } = await import("@tauri-apps/api/event");
      const mainWindow = await WebviewWindow.getByLabel("main");
      if (mainWindow) {
        await emit("open-chat", { text });
        await mainWindow.show();
        await mainWindow.setFocus();
      }
      await getCurrentWindow().hide();
    }
    setChatInput("");
  };

  // Sort: active tasks first, completed (optimistic) at bottom
  const isTaskCompleted = useCallback(
    (t: TodayTask) => t.completed || completedIds.has(t.id),
    [completedIds],
  );
  const sortedTasks = useMemo(
    () =>
      [...todayTasks].sort((a, b) => (isTaskCompleted(a) ? 1 : 0) - (isTaskCompleted(b) ? 1 : 0)),
    [todayTasks, isTaskCompleted],
  );
  const activeCount = useMemo(
    () => sortedTasks.filter((t) => !isTaskCompleted(t)).length,
    [sortedTasks, isTaskCompleted],
  );

  useTransparentBackground();

  // Auto-resize window to match content height
  const contentRef = useRef<HTMLDivElement>(null);
  const lastHeight = useRef(0);

  const TRAY_WIDTH = 320;

  const syncSize = useCallback((entries?: ResizeObserverEntry[]) => {
    if (!isTauri) return;

    const h = Math.ceil(
      entries?.[0]?.contentRect.height ?? contentRef.current?.getBoundingClientRect().height ?? 0,
    );
    if (h === lastHeight.current || h <= 0) return;
    lastHeight.current = h;

    getCurrentWindow()
      .setSize(new LogicalSize(TRAY_WIDTH, h))
      .catch((e: unknown) => console.error("tray resize failed", e));
  }, []);

  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;

    const observer = new ResizeObserver(syncSize);
    observer.observe(el);
    syncSize();
    return () => observer.disconnect();
  }, [syncSize]);

  return (
    <div className="w-screen text-primary">
      <div
        ref={contentRef}
        className="w-full rounded-2xl overflow-hidden bg-surface-floating shadow-2xl shadow-black/50 border border-border-subtle"
      >
        {/* Header */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border">
          <div className="w-7 h-7 rounded-md bg-white flex items-center justify-center p-0.5">
            <KlyntLogo className="w-full h-full" />
          </div>
          <div className="flex-1">
            <p className="text-[13px] font-light text-primary">Klynt Agent</p>
            <div className="flex items-center gap-1.5">
              <div
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: statusColors[displayStatus] }}
              />
              <span className="text-[11px] text-muted font-light capitalize">{displayStatus}</span>
            </div>
          </div>
        </div>

        {/* Today's Tasks */}
        <div className="px-4 py-3 border-b border-border overflow-y-auto max-h-[240px]">
          <div className="flex items-center justify-between mb-2">
            <span className="text-[11px] text-muted font-light uppercase tracking-wider">
              Today
            </span>
            {activeCount > 0 && (
              <span className="text-[11px] text-muted font-light">{activeCount}</span>
            )}
          </div>
          {sortedTasks.length === 0 ? (
            <p className="text-[12px] text-dim font-light py-2">No tasks for today</p>
          ) : (
            <div className="space-y-0.5">
              {sortedTasks.map((task) => {
                const done = isTaskCompleted(task);
                return (
                  <div
                    key={task.id}
                    className={`flex items-center gap-2.5 py-1.5 px-2 rounded-md border-l-2 border-y-0 border-r-0 hover:bg-surface-base/50 transition-colors ${taskIndicatorClass(task, done)}`}
                  >
                    <Checkbox checked={done} onCheckedChange={() => handleToggleTask(task.id)} />
                    <span
                      className={`flex-1 text-[12px] font-light truncate ${
                        done
                          ? "text-dim line-through"
                          : task.isOverdue
                            ? "text-primary"
                            : "text-secondary"
                      }`}
                    >
                      {task.title}
                    </span>
                    {!done && task.priority && (
                      <Badge
                        variant="priority"
                        value={task.priority}
                        className="text-[10px] px-1.5 py-0"
                      />
                    )}
                    {!done && task.dueDisplay && (
                      <span
                        className={`text-[10px] font-light flex-shrink-0 ${
                          task.isOverdue ? "text-destructive" : "text-muted"
                        }`}
                      >
                        {task.dueDisplay}
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Calendar Events */}
        <div className="px-4 py-3 border-b border-border">
          <span className="text-[11px] text-muted font-light uppercase tracking-wider">
            Upcoming
          </span>
          <div className="mt-2 space-y-2">
            {calendarEvents.map((event) => (
              <div key={event.id} className="flex items-center gap-2.5">
                <div
                  className="w-1 h-6 rounded-full flex-shrink-0"
                  style={{ backgroundColor: event.color }}
                />
                <div className="flex-1 min-w-0">
                  <p className="text-[12px] font-light text-secondary truncate">{event.title}</p>
                </div>
                <span className="text-[11px] text-muted font-light flex-shrink-0">
                  {new Date(event.startAt).toLocaleTimeString([], {
                    hour: "numeric",
                    minute: "2-digit",
                  })}
                </span>
              </div>
            ))}
          </div>
        </div>

        {/* Chat Input */}
        <div className="px-3 py-3">
          <div className="flex items-center gap-2 bg-surface-base rounded-xl px-3 py-2">
            <input
              type="text"
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleChatSubmit();
                }
              }}
              placeholder="Ask Klynt\u2026"
              aria-label="Ask Klynt"
              className="flex-1 bg-transparent text-[13px] font-light text-primary placeholder:text-muted"
            />
            <button
              type="button"
              onClick={handleChatSubmit}
              disabled={!chatInput.trim()}
              className="w-6 h-6 rounded-md flex items-center justify-center text-muted hover:text-primary transition-colors disabled:opacity-30"
            >
              <Send className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
