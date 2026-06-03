import Check from "lucide-react/dist/esm/icons/check";
import Lightbulb from "lucide-react/dist/esm/icons/lightbulb";
import LogOut from "lucide-react/dist/esm/icons/log-out";
import Monitor from "lucide-react/dist/esm/icons/monitor";
import Settings from "lucide-react/dist/esm/icons/settings";
import X from "lucide-react/dist/esm/icons/x";
import XCircle from "lucide-react/dist/esm/icons/x-circle";
import { useCallback, useMemo, useRef } from "react";
import { cn } from "@/utils/cn";
import { useTransparentBackground } from "@/hooks/window/useTransparentBackground";
import { useWindowAutoResize } from "@/hooks/window/useWindowAutoResize";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
import { emit, getCurrentWindow, getWindowByLabel, ipc, isTauri } from "@/utils/tauri-bridge";
import { useFocusTimer } from "../hooks/useFocusTimer";
import { todayISO } from "../lib/dates";
import type { CalendarEvent, TodayTask } from "../types";
import { Badge } from "./Badge";
import { Checkbox } from "./Checkbox";
import { FocusControl } from "./FocusControl";
import { useCoachingNudge } from "./stubs";


const MAX_TRAY_HEIGHT = 800;
const TRAY_WIDTH = 320;
const TRAY_MIN_HEIGHT = 360;

export function Tray() {
  const todayTasksQuery = useTauriQuery<TodayTask[]>({
    queryKey: qk.tasks.today(),
    command: "today_tasks",
    fallback: [],
  });
  const todayTasks = todayTasksQuery.data;
  const dateKey = todayISO();
  const calendarQuery = useTauriQuery<CalendarEvent[]>({
    queryKey: qk.calendar.eventsForDate(dateKey),
    command: "productivity_calendar_events",
    args: { date: dateKey },
    fallback: [],
  });
  const calendarEvents = calendarQuery.data;

  const { nudge: coachingNudge, handleFeedback: handleCoachingFeedback } = useCoachingNudge({
    autoCollapseMs: 30_000,
  });

  const toggleComplete = useTauriMutation<TodayTask, { id: string }>({
    command: "task_toggle_complete",
    optimistic: {
      queryKey: qk.tasks.today(),
      update: (vars, prev: TodayTask[] = []) =>
        prev.map((t) => (t.id === vars.id ? { ...t, completed: !t.completed } : t)),
    },
  });

  const focusTimer = useFocusTimer();

  const handleToggleTask = async (taskId: string) => {
    await toggleComplete.mutate({ id: taskId });
  };

  // Mirrors .bak: emit `navigate` so the main window's router picks the path,
  // THEN raise the main window via its label, THEN hide the tray. The tray's
  // own `getCurrentWindow()` only knows itself — without `getWindowByLabel`
  // the main window stays hidden and the navigate event has no audience.
  const navigateMain = async (path: string) => {
    if (!isTauri()) return;
    try {
      await emit("navigate", { path });
      const main = getWindowByLabel("main");
      await main.show();
      await main.setFocus();
      await getCurrentWindow().hide();
    } catch {
      // silent — dev/browser
    }
  };

  const handleOpenTask = (taskId: string) => navigateMain(`/task/${taskId}`);
  const handleOpenSettings = () => navigateMain("/settings");

  const handleOpenDashboard = async () => {
    try {
      await ipc("show_dashboard");
    } catch {
      // silent
    }
    if (isTauri()) {
      try {
        await getCurrentWindow().hide();
      } catch {
        // silent
      }
    }
  };

  const handleOpenGitHub = () => {
    ipc("open_url", { url: "https://github.com/KlyntLabs/klyntbot" }).catch(() => {});
  };

  const handleQuit = () => {
    ipc("quit_app").catch(() => {});
  };

  const isTaskCompleted = useCallback((t: TodayTask) => t.completed, []);
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
  const contentRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  useWindowAutoResize(bodyRef, {
    width: TRAY_WIDTH,
    minHeight: TRAY_MIN_HEIGHT,
    maxHeight: MAX_TRAY_HEIGHT,
  });

  return (
    <div className="w-full h-full flex text-text-primary font-ui text-ui-sm">
      <div ref={contentRef} className="w-full flex-1 flex flex-col overflow-hidden">
        <div ref={bodyRef} className="overflow-visible">
          {coachingNudge && (
            <div className="py-3 px-4">
              <div className="flex items-start gap-2.5">
                <Lightbulb className="w-3.5 h-3.5 text-[var(--tray-info)]" strokeWidth={1.5} />
                <p className="flex-1 text-ui-xs font-light leading-normal text-text-muted m-0">{coachingNudge.message}</p>
              </div>
              <div className="flex gap-1 mt-2 ml-6">
                <button
                  type="button"
                  onClick={() => handleCoachingFeedback(coachingNudge.id, "helpful")}
                  className="tray-nudge-action is-success"
                >
                  <Check className="w-3.5 h-3.5" strokeWidth={2} />
                  Helpful
                </button>
                <button
                  type="button"
                  onClick={() => handleCoachingFeedback(coachingNudge.id, "dismissed")}
                  className="h-6 px-2 flex items-center gap-1 rounded-md bg-transparent border-none text-text-muted text-ui-3xs cursor-pointer"
                >
                  <X className="w-3.5 h-3.5" strokeWidth={2} />
                  Dismiss
                </button>
                <button
                  type="button"
                  onClick={() => handleCoachingFeedback(coachingNudge.id, "stop")}
                  className="tray-nudge-action is-danger"
                >
                  <XCircle className="w-3.5 h-3.5" strokeWidth={2} />
                  Stop
                </button>
              </div>
              <div className="mx-4 h-px" />
            </div>
          )}

          <FocusControl timer={focusTimer} />

          {sortedTasks.length > 0 && (
            <>
              <div className="mx-4 h-px" />
              <div className="py-3 px-4 overflow-visible">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-ui-2xs font-light text-text-muted uppercase tracking-[0.08em]">Today</span>
                  {activeCount > 0 && <Badge variant="muted">{activeCount}</Badge>}
                </div>
                <div className="flex flex-col gap-0.5">
                  {sortedTasks.map((task) => {
                    const done = isTaskCompleted(task);
                    return (
                      <div key={task.id} className={cn("flex items-center gap-2.5 py-1.5 px-2 rounded-lg border-l-2 border-transparent", done && "border-[var(--text-dim)]", task.isOverdue && !done && "border-[var(--tray-destructive)]", task.isDueToday && !done && "border-[var(--tray-brand)]", task.status === "doing" && !done && "border-[var(--tray-info)]")}>
                        <Checkbox
                          checked={done}
                          onCheckedChange={() => handleToggleTask(task.id)}
                        />
                        <button
                          type="button"
                          onClick={() => handleOpenTask(task.id)}
                          className={`tray-task-title${done ? " is-done" : ""}${
                            task.isOverdue && !done ? " is-overdue" : ""
                          }`}
                        >
                          {task.title}
                        </button>
                        {!done && task.priority && <Badge variant="brand">{task.priority}</Badge>}
                        {!done && task.dueDisplay && (
                          <span className={cn("text-ui-3xs font-light text-text-muted shrink-0", task.isOverdue && "text-[var(--tray-destructive)]")}>
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

          {calendarEvents.length > 0 && (
            <>
              <div className="mx-4 h-px" />
              <div className="py-3 px-4">
                <span className="text-ui-2xs font-light text-text-muted uppercase tracking-[0.08em]">Upcoming</span>
                <div className="flex flex-col gap-2 mt-2">
                  {calendarEvents.map((event) => (
                    <div key={event.id} className="flex items-center gap-2.5">
                      <div
                        className="w-1 h-6 rounded-sm shrink-0"
                        style={{
                          backgroundColor: event.color ?? "var(--tray-brand)",
                        }}
                      />
                      <div className="flex-1 min-w-0">
                        <p className="text-ui-xs font-light text-text-muted whitespace-nowrap overflow-hidden text-ellipsis m-0">{event.title}</p>
                      </div>
                      <span className="text-ui-2xs font-light text-text-dim shrink-0">
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

          <div className="mx-4 h-px" />

          <div className="flex items-center justify-end gap-0.5 py-1.5 px-2.5">
            <button
              type="button"
              onClick={handleOpenDashboard}
              title="Open Dashboard"
              className="w-[30px] h-[30px] flex items-center justify-center rounded-lg bg-transparent border-none text-text-faint cursor-pointer"
            >
              <Monitor className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
            <button
              type="button"
              onClick={handleOpenSettings}
              title="Settings"
              className="w-[30px] h-[30px] flex items-center justify-center rounded-lg bg-transparent border-none text-text-faint cursor-pointer"
            >
              <Settings className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
            <button
              type="button"
              onClick={handleOpenGitHub}
              title="GitHub"
              className="w-[30px] h-[30px] flex items-center justify-center rounded-lg bg-transparent border-none text-text-faint cursor-pointer"
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
              className="w-[30px] h-[30px] flex items-center justify-center rounded-lg bg-transparent border-none text-text-faint cursor-pointer hover:text-[var(--tray-destructive)] hover:bg-[var(--tray-destructive-bg)]"
            >
              <LogOut className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
