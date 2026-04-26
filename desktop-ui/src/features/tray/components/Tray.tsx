import {
	Check,
	Lightbulb,
	LogOut,
	Monitor,
	Settings,
	X,
	XCircle,
} from "lucide-react";
import { useCallback, useMemo, useRef } from "react";
import { useTransparentBackground } from "@/hooks/window/useTransparentBackground";
import { useWindowAutoResize } from "@/hooks/window/useWindowAutoResize";
import {
	emit,
	getCurrentWindow,
	ipc,
	isTauri,
} from "@/utils/tauri-bridge";
import { useFocusTimer } from "../hooks/useFocusTimer";
import { useSetToggle } from "../hooks/useSetToggle";
import { useTrayMutation } from "../hooks/useTrayMutation";
import { useTrayQuery } from "../hooks/useTrayQuery";
import { todayISO } from "../lib/dates";
import type { CalendarEvent, TodayTask } from "../types";
import { Badge } from "./Badge";
import { Checkbox } from "./Checkbox";
import { FocusControl } from "./FocusControl";
import { useCoachingNudge } from "./stubs";

import "../tray.css";

const MAX_TRAY_HEIGHT = 800;
const TRAY_WIDTH = 320;
const TRAY_MIN_HEIGHT = 360;

function taskIndicatorClass(task: TodayTask, isCompleted: boolean): string {
	if (isCompleted) return "is-dim";
	if (task.isOverdue) return "is-overdue";
	if (task.isDueToday) return "is-today";
	if (task.status === "doing") return "is-doing";
	return "";
}

export function Tray() {
	const todayTasksQuery = useTrayQuery<TodayTask[]>(
		"today_tasks",
		undefined,
		[],
	);
	const todayTasks = todayTasksQuery.data;
	const calendarQuery = useTrayQuery<CalendarEvent[]>(
		"productivity_calendar_events",
		{ date: todayISO() },
		[],
	);
	const calendarEvents = calendarQuery.data;

	const { nudge: coachingNudge, handleFeedback: handleCoachingFeedback } =
		useCoachingNudge({ autoCollapseMs: 30_000 });

	const toggleComplete = useTrayMutation<TodayTask, { id: string }>(
		"task_toggle_complete",
	);
	const [completedIds, toggleCompletedId] = useSetToggle();

	const focusTimer = useFocusTimer();

	const handleToggleTask = async (taskId: string) => {
		toggleCompletedId(taskId);
		await toggleComplete.mutate({ id: taskId });
	};

	const handleOpenTask = async (taskId: string) => {
		if (!isTauri()) return;
		try {
			await emit("navigate", { path: `/task/${taskId}` });
			await getCurrentWindow().hide();
		} catch {
			// silent
		}
	};

	const handleOpenSettings = async () => {
		if (!isTauri()) return;
		try {
			await emit("navigate", { path: "/settings" });
			await getCurrentWindow().hide();
		} catch {
			// silent
		}
	};

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
		ipc("open_url", { url: "https://github.com/KlyntLabs/klyntbot" }).catch(
			() => {},
		);
	};

	const handleQuit = () => {
		ipc("quit_app").catch(() => {});
	};

	const isTaskCompleted = useCallback(
		(t: TodayTask) => t.completed || completedIds.has(t.id),
		[completedIds],
	);
	const sortedTasks = useMemo(
		() =>
			[...todayTasks].sort(
				(a, b) => (isTaskCompleted(a) ? 1 : 0) - (isTaskCompleted(b) ? 1 : 0),
			),
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
		<div className="tray-root">
			<div ref={contentRef} className="tray-shell">
				<div ref={bodyRef} className="tray-body">
					{coachingNudge && (
						<div className="tray-nudge">
							<div className="tray-nudge-row">
								<Lightbulb className="tray-icon-xs is-info" strokeWidth={1.5} />
								<p className="tray-nudge-text">{coachingNudge.message}</p>
							</div>
							<div className="tray-nudge-actions">
								<button
									type="button"
									onClick={() =>
										handleCoachingFeedback(coachingNudge.id, "helpful")
									}
									className="tray-nudge-action is-success"
								>
									<Check className="tray-icon-xs" strokeWidth={2} />
									Helpful
								</button>
								<button
									type="button"
									onClick={() =>
										handleCoachingFeedback(coachingNudge.id, "dismissed")
									}
									className="tray-nudge-action"
								>
									<X className="tray-icon-xs" strokeWidth={2} />
									Dismiss
								</button>
								<button
									type="button"
									onClick={() =>
										handleCoachingFeedback(coachingNudge.id, "stop")
									}
									className="tray-nudge-action is-danger"
								>
									<XCircle className="tray-icon-xs" strokeWidth={2} />
									Stop
								</button>
							</div>
							<div className="tray-divider" />
						</div>
					)}

					<FocusControl timer={focusTimer} />

					{sortedTasks.length > 0 && (
						<>
							<div className="tray-divider" />
							<div className="tray-section tray-tasks">
								<div className="tray-section-header">
									<span className="tray-section-title">Today</span>
									{activeCount > 0 && (
										<Badge variant="muted">{activeCount}</Badge>
									)}
								</div>
								<div className="tray-task-list">
									{sortedTasks.map((task) => {
										const done = isTaskCompleted(task);
										return (
											<div
												key={task.id}
												className={`tray-task ${taskIndicatorClass(task, done)}`}
											>
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
												{!done && task.priority && (
													<Badge variant="brand">{task.priority}</Badge>
												)}
												{!done && task.dueDisplay && (
													<span
														className={`tray-task-due${
															task.isOverdue ? " is-overdue" : ""
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

					{calendarEvents.length > 0 && (
						<>
							<div className="tray-divider" />
							<div className="tray-section">
								<span className="tray-section-title">Upcoming</span>
								<div className="tray-event-list">
									{calendarEvents.map((event) => (
										<div key={event.id} className="tray-event">
											<div
												className="tray-event-color"
												style={{
													backgroundColor: event.color ?? "var(--brand)",
												}}
											/>
											<div className="tray-event-body">
												<p className="tray-event-title">{event.title}</p>
											</div>
											<span className="tray-event-time">
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

					<div className="tray-divider" />

					<div className="tray-footer">
						<button
							type="button"
							onClick={handleOpenDashboard}
							title="Open Dashboard"
							className="tray-footer-btn"
						>
							<Monitor className="tray-icon-xs" strokeWidth={1.5} />
						</button>
						<button
							type="button"
							onClick={handleOpenSettings}
							title="Settings"
							className="tray-footer-btn"
						>
							<Settings className="tray-icon-xs" strokeWidth={1.5} />
						</button>
						<button
							type="button"
							onClick={handleOpenGitHub}
							title="GitHub"
							className="tray-footer-btn"
						>
							<svg
								className="tray-icon-xs"
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
							className="tray-footer-btn is-danger"
						>
							<LogOut className="tray-icon-xs" strokeWidth={1.5} />
						</button>
					</div>
				</div>
			</div>
		</div>
	);
}
