import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Check, Lightbulb, LogOut, Monitor, Send, Settings, X, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useCoachingNudge } from "../../hooks/useCoachingNudge";
import { useEvent } from "../../hooks/useEvent";
import { ipc } from "../../hooks/useIpc";
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

  // Coaching nudge (Channel 2: tray nudge) — 30s auto-collapse for compact tray
  const { nudge: coachingNudge, handleFeedback: handleCoachingFeedback } = useCoachingNudge({
    autoCollapseMs: 30_000,
  });

  const toggleComplete = useMutation<TodayTask, { id: string }>("task_toggle_complete");
  const [chatInput, setChatInput] = useState("");
  const [completedIds, toggleCompletedId] = useSetToggle();

  // Auto-refresh when entities change
  useEvent<{ entityKind: string; id: string }>("entity:updated", () => {
    refetchTasks();
  });

  const displayStatus: DisplayStatus = agentStatusData.status === "active" ? "active" : "idle";

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

  const handleOpenSettings = async () => {
    if (isTauri) {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const { emit } = await import("@tauri-apps/api/event");
      const mainWindow = await WebviewWindow.getByLabel("main");
      if (mainWindow) {
        await emit("navigate", { path: "/settings" });
        await mainWindow.show();
        await mainWindow.setFocus();
      }
      await getCurrentWindow().hide();
    }
  };

  const handleOpenDashboard = async () => {
    await ipc("show_dashboard");
    if (isTauri) {
      await getCurrentWindow().hide();
    }
  };

  const handleOpenGitHub = () => {
    ipc("open_url", { url: "https://github.com/KlyntLabs/klyntbot" });
  };

  const handleQuit = () => {
    ipc("quit_app");
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
        className="w-full glass-floating overflow-hidden"
        style={{ animation: "glass-appear 0.2s ease-out" }}
      >
        <div className="rounded-[var(--glass-radius-inner)] overflow-hidden">
          {/* Header */}
          <div className="flex items-center gap-3 px-4 py-3">
            <div className="w-7 h-7 rounded-lg bg-white/90 flex items-center justify-center p-0.5 ">
              <KlyntLogo className="w-full h-full" />
            </div>
            <div className="flex-1">
              <p className="text-[13px] font-light text-primary">Klynt Agent</p>
              <div className="flex items-center gap-1.5">
                <div
                  className="w-1.5 h-1.5 rounded-full"
                  style={{
                    backgroundColor: displayStatus === "active" ? "var(--success)" : "var(--brand)",
                  }}
                />
                <span className="text-[11px] text-muted font-light capitalize">
                  {displayStatus}
                </span>
              </div>
            </div>
          </div>

          {/* Coaching Nudge (Channel 2: tray) */}
          {coachingNudge && (
            <>
              <div className="mx-4 glass-divider" />
              <div className="px-4 py-3" style={{ animation: "nudge-slide-in 0.25s ease-out" }}>
                <div className="flex items-start gap-2.5">
                  <Lightbulb className="w-3.5 h-3.5 text-info shrink-0 mt-0.5" strokeWidth={1.5} />
                  <p className="flex-1 text-[12px] text-secondary font-light leading-relaxed">
                    {coachingNudge.message}
                  </p>
                </div>
                <div className="flex items-center gap-1 mt-2 ml-6">
                  <button
                    type="button"
                    onClick={() => handleCoachingFeedback(coachingNudge.id, "helpful")}
                    title="Helpful"
                    className="h-6 px-2 flex items-center gap-1 rounded-md text-[10px] text-muted hover:text-success hover:bg-white/[0.06] transition-colors"
                  >
                    <Check className="w-3 h-3" strokeWidth={2} />
                    Helpful
                  </button>
                  <button
                    type="button"
                    onClick={() => handleCoachingFeedback(coachingNudge.id, "dismissed")}
                    title="Dismiss"
                    className="h-6 px-2 flex items-center gap-1 rounded-md text-[10px] text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
                  >
                    <X className="w-3 h-3" strokeWidth={2} />
                    Dismiss
                  </button>
                  <button
                    type="button"
                    onClick={() => handleCoachingFeedback(coachingNudge.id, "stop")}
                    title="Stop suggesting this"
                    className="h-6 px-2 flex items-center gap-1 rounded-md text-[10px] text-muted hover:text-destructive hover:bg-white/[0.06] transition-colors"
                  >
                    <XCircle className="w-3 h-3" strokeWidth={2} />
                    Stop
                  </button>
                </div>
              </div>
            </>
          )}

          <div className="mx-4 glass-divider" />

          {/* Today's Tasks */}
          <div className="px-4 py-3 overflow-y-auto max-h-[240px]">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[11px] text-muted font-light uppercase tracking-wider">
                Today
              </span>
              {activeCount > 0 && (
                <span className="glass-badge px-2 py-0.5 text-[10px] text-muted font-light">
                  {activeCount}
                </span>
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
                      className={`flex items-center gap-2.5 py-1.5 px-2 rounded-lg border-l-2 border-y-0 border-r-0 hover:bg-white/[0.04] transition-colors ${taskIndicatorClass(task, done)}`}
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

          <div className="mx-4 glass-divider" />

          {/* Calendar Events */}
          <div className="px-4 py-3">
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
                  <span className="text-[11px] text-dim font-light flex-shrink-0">
                    {new Date(event.startAt).toLocaleTimeString([], {
                      hour: "numeric",
                      minute: "2-digit",
                    })}
                  </span>
                </div>
              ))}
            </div>
          </div>

          <div className="mx-4 glass-divider" />

          {/* Chat Input */}
          <div className="px-3 py-3">
            <div className="glass-input flex items-center gap-2 px-3 py-2">
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
                className="flex-1 bg-transparent text-[13px] font-light text-primary placeholder:text-muted outline-none"
              />
              <button
                type="button"
                onClick={handleChatSubmit}
                disabled={!chatInput.trim()}
                className="w-6 h-6 rounded-lg flex items-center justify-center text-muted hover:text-primary transition-all disabled:opacity-30"
              >
                <Send className="w-3.5 h-3.5" strokeWidth={1.5} />
              </button>
            </div>
          </div>

          <div className="mx-4 glass-divider" />

          {/* Footer Menu */}
          <div className="flex items-center justify-end gap-1 px-3 py-2">
            <button
              type="button"
              onClick={handleOpenDashboard}
              title="Open Dashboard"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <Monitor className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
            <button
              type="button"
              onClick={handleOpenSettings}
              title="Settings"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <Settings className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
            <button
              type="button"
              onClick={handleOpenGitHub}
              title="GitHub"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <svg
                className="w-3.5 h-3.5"
                viewBox="0 0 24 24"
                fill="currentColor"
                aria-hidden="true"
              >
                <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2Z" />
              </svg>
            </button>
            <button
              type="button"
              onClick={handleQuit}
              title="Quit Klynt"
              className="w-7 h-7 flex items-center justify-center rounded-lg text-muted hover:text-destructive hover:bg-white/[0.06] transition-colors"
            >
              <LogOut className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
