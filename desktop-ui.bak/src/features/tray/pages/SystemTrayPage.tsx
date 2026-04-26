import { useCoachingNudge } from "@shared/hooks/useCoachingNudge";
import { useFocusTimer } from "@shared/hooks/useFocusTimer";
import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { useSetToggle } from "@shared/hooks/useSetToggle";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";
import { todayISO } from "@shared/lib/dates";
import { isTauri } from "@shared/lib/utils";
import type { CalendarEvent, TodayTask } from "@shared/types";
import { Badge, Checkbox } from "@shared/ui";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { Check, Lightbulb, LogOut, Monitor, Settings, X, XCircle } from "lucide-react";
import { useCallback, useMemo, useRef } from "react";
import { FocusControl } from "../components/FocusControl";

/** Color of the left border indicator based on task state */
function taskIndicatorClass(task: TodayTask, isCompleted: boolean): string {
  if (isCompleted) return "border-l-dim";
  if (task.isOverdue) return "border-l-destructive";
  if (task.isDueToday) return "border-l-brand";
  if (task.status === "doing") return "border-l-info";
  return "border-l-transparent";
}

export function SystemTray() {
  const { data: todayTasks } = useQuery<TodayTask[]>("today_tasks", undefined, [], {
    invalidateOn: ["entity:updated"],
  });
  const { data: calendarEvents } = useQuery<CalendarEvent[]>(
    "productivity_calendar_events",
    { date: todayISO() },
    [],
  );

  // Coaching nudge (Channel 2: tray nudge) — 30s auto-collapse for compact tray
  const { nudge: coachingNudge, handleFeedback: handleCoachingFeedback } = useCoachingNudge({
    autoCollapseMs: 30_000,
  });

  const toggleComplete = useMutation<TodayTask, { id: string }>("task_toggle_complete");
  const [completedIds, toggleCompletedId] = useSetToggle();

  const focusTimer = useFocusTimer();

  const handleToggleTask = async (taskId: string) => {
    toggleCompletedId(taskId);
    await toggleComplete.mutate({ id: taskId });
  };

  const handleOpenTask = async (taskId: string) => {
    if (isTauri) {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const { emit } = await import("@tauri-apps/api/event");
      const mainWindow = await WebviewWindow.getByLabel("main");
      if (mainWindow) {
        await emit("navigate", { path: `/task/${taskId}` });
        await mainWindow.show();
        await mainWindow.setFocus();
      }
      await getCurrentWindow().hide();
    }
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

  useTransparentBackground({ nativeVibrancy: true });

  const contentRef = useRef<HTMLDivElement>(null);
  const MAX_TRAY_HEIGHT = 800;
  useWindowAutoResize(contentRef, { width: 320, maxHeight: MAX_TRAY_HEIGHT });

  return (
    <div className="w-screen text-foreground">
      <div
        ref={contentRef}
        className="w-full glass-floating overflow-hidden"
        style={{ animation: "glass-appear 0.2s ease-out" }}
      >
        <div
          className="rounded-[var(--glass-radius-inner)] overflow-y-auto"
          style={{ maxHeight: MAX_TRAY_HEIGHT }}
        >
          {/* Coaching Nudge (Channel 2: tray) */}
          {coachingNudge && (
            <div className="px-4 py-3" style={{ animation: "nudge-slide-in 0.25s ease-out" }}>
              <div className="flex items-start gap-2.5">
                <Lightbulb className="size-3.5 text-info shrink-0 mt-0.5" strokeWidth={1.5} />
                <p className="flex-1 text-xs text-muted-foreground font-light leading-relaxed">
                  {coachingNudge.message}
                </p>
              </div>
              <div className="flex items-center gap-1 mt-2 ml-6">
                <button
                  type="button"
                  onClick={() => handleCoachingFeedback(coachingNudge.id, "helpful")}
                  title="Helpful"
                  className="h-6 px-2 flex items-center gap-1 rounded-md text-2xs text-muted-foreground hover:text-success hover:bg-accent transition-colors"
                >
                  <Check className="size-3" strokeWidth={2} />
                  Helpful
                </button>
                <button
                  type="button"
                  onClick={() => handleCoachingFeedback(coachingNudge.id, "dismissed")}
                  title="Dismiss"
                  className="h-6 px-2 flex items-center gap-1 rounded-md text-2xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                >
                  <X className="size-3" strokeWidth={2} />
                  Dismiss
                </button>
                <button
                  type="button"
                  onClick={() => handleCoachingFeedback(coachingNudge.id, "stop")}
                  title="Stop suggesting this"
                  className="h-6 px-2 flex items-center gap-1 rounded-md text-2xs text-muted-foreground hover:text-destructive hover:bg-accent transition-colors"
                >
                  <XCircle className="size-3" strokeWidth={2} />
                  Stop
                </button>
              </div>
              <div className="mx-4 mt-3 glass-divider" />
            </div>
          )}

          {/* Focus Timer */}
          <FocusControl timer={focusTimer} />

          {/* Today's Tasks (hidden when empty) */}
          {sortedTasks.length > 0 && (
            <>
              <div className="mx-4 glass-divider" />
              <div className="px-4 py-3 overflow-y-auto max-h-[240px]">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-[11px] text-muted-foreground font-light uppercase tracking-wider">
                    Today
                  </span>
                  {activeCount > 0 && (
                    <span className="glass-badge px-2 py-0.5 text-2xs text-muted-foreground font-light">
                      {activeCount}
                    </span>
                  )}
                </div>
                <div className="space-y-0.5">
                  {sortedTasks.map((task) => {
                    const done = isTaskCompleted(task);
                    return (
                      <div
                        key={task.id}
                        className={`flex items-center gap-2.5 py-1.5 px-2 rounded-lg border-l-2 border-y-0 border-r-0 hover:bg-accent transition-colors ${taskIndicatorClass(task, done)}`}
                      >
                        <Checkbox
                          checked={done}
                          onCheckedChange={() => handleToggleTask(task.id)}
                        />
                        <button
                          type="button"
                          onClick={() => handleOpenTask(task.id)}
                          className={`flex-1 text-xs font-light truncate text-left hover:underline ${
                            done
                              ? "text-dim line-through"
                              : task.isOverdue
                                ? "text-foreground"
                                : "text-muted-foreground"
                          }`}
                        >
                          {task.title}
                        </button>
                        {!done && task.priority && (
                          <Badge variant="brand" className="text-2xs px-1.5 py-0">
                            {task.priority}
                          </Badge>
                        )}
                        {!done && task.dueDisplay && (
                          <span
                            className={`text-2xs font-light flex-shrink-0 ${
                              task.isOverdue ? "text-destructive" : "text-muted-foreground"
                            }`}
                          >
                            {task.dueDisplay}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            </>
          )}

          {/* Calendar Events (hidden when empty) */}
          {calendarEvents.length > 0 && (
            <>
              <div className="mx-4 glass-divider" />
              <div className="px-4 py-3">
                <span className="text-[11px] text-muted-foreground font-light uppercase tracking-wider">
                  Upcoming
                </span>
                <div className="mt-2 space-y-2">
                  {calendarEvents.map((event) => (
                    <div key={event.id} className="flex items-center gap-2.5">
                      <div
                        className="w-1 h-6 rounded-full flex-shrink-0"
                        style={{ backgroundColor: event.color ?? "var(--brand)" }}
                      />
                      <div className="flex-1 min-w-0">
                        <p className="text-xs font-light text-muted-foreground truncate">
                          {event.title}
                        </p>
                      </div>
                      <span className="text-[11px] text-dim font-light flex-shrink-0">
                        {new Date(event.startedAt).toLocaleTimeString([], {
                          hour: "numeric",
                          minute: "2-digit",
                        })}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </>
          )}

          <div className="mx-4 glass-divider" />

          {/* Footer Menu */}
          <div className="flex items-center justify-end gap-1 px-3 py-2">
            <button
              type="button"
              onClick={handleOpenDashboard}
              title="Open Dashboard"
              className="size-7 flex items-center justify-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <Monitor className="size-3.5" strokeWidth={1.5} />
            </button>
            <button
              type="button"
              onClick={handleOpenSettings}
              title="Settings"
              className="size-7 flex items-center justify-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <Settings className="size-3.5" strokeWidth={1.5} />
            </button>
            <button
              type="button"
              onClick={handleOpenGitHub}
              title="GitHub"
              className="size-7 flex items-center justify-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <svg className="size-3.5" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2Z" />
              </svg>
            </button>
            <button
              type="button"
              onClick={handleQuit}
              title="Quit Klynt"
              className="size-7 flex items-center justify-center rounded-lg text-muted-foreground hover:text-destructive hover:bg-accent transition-colors"
            >
              <LogOut className="size-3.5" strokeWidth={1.5} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
