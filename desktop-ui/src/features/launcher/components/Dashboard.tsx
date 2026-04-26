import { useLauncherState } from "../store";
import type { DashboardData } from "../types";

interface DashboardProps {
	onOpenTask?: (taskId: string) => void;
}

function formatTime(iso: string): string {
	try {
		return new Date(iso).toLocaleTimeString([], {
			hour: "numeric",
			minute: "2-digit",
		});
	} catch {
		return "";
	}
}

export function Dashboard({ onOpenTask }: DashboardProps) {
	const dashboard = useLauncherState((s) => s.dashboard);

	if (!dashboard) {
		return <div className="lc-dash-loading">Loading...</div>;
	}

	const hasContent =
		dashboard.calendar.length > 0 ||
		dashboard.tasks.length > 0 ||
		dashboard.productivity.totalMinutes > 0;

	if (!hasContent) {
		return (
			<div className="lc-dash-empty">
				<p className="lc-muted-sm">No activity yet today</p>
				<p className="lc-dim-xs">
					Start a focus session or search for anything
				</p>
			</div>
		);
	}

	return (
		<div className="lc-dash">
			{dashboard.calendar.length > 0 && (
				<CalendarWidget events={dashboard.calendar} />
			)}
			{dashboard.tasks.length > 0 && (
				<TasksWidget tasks={dashboard.tasks} onOpenTask={onOpenTask} />
			)}
			{dashboard.productivity.totalMinutes > 0 && (
				<ProductivityWidget productivity={dashboard.productivity} />
			)}
		</div>
	);
}

function CalendarWidget({ events }: { events: DashboardData["calendar"] }) {
	return (
		<div className="lc-card">
			<div className="lc-card-header">
				<span className="lc-card-title">Upcoming</span>
			</div>
			<div className="lc-card-body">
				{events.map((event) => {
					const isNow = event.minutesUntil <= 0;
					const isSoon = event.minutesUntil > 0 && event.minutesUntil <= 15;
					const dotClass = isNow ? "is-now" : isSoon ? "is-soon" : "";
					return (
						<div key={event.eventId} className="lc-cal-row">
							<div className={`lc-cal-dot ${dotClass}`} />
							<span className="lc-cal-title">{event.title}</span>
							<span className={`lc-cal-time ${dotClass}`}>
								{isNow ? "Now" : `${event.minutesUntil}m`}
							</span>
						</div>
					);
				})}
			</div>
		</div>
	);
}

function TasksWidget({
	tasks,
	onOpenTask,
}: {
	tasks: DashboardData["tasks"];
	onOpenTask?: (id: string) => void;
}) {
	return (
		<div className="lc-card">
			<div className="lc-card-header">
				<span className="lc-card-title">Tasks</span>
				<span className="lc-card-count">{tasks.length}</span>
			</div>
			<div className="lc-card-body">
				{tasks.map((task) => (
					<button
						key={task.id}
						type="button"
						onClick={() => onOpenTask?.(task.id)}
						className="lc-task-row"
					>
						<div className={`lc-task-dot status-${task.status}`} />
						<span className="lc-task-title">{task.title}</span>
						{task.dueDate && (
							<span className="lc-task-time">{formatTime(task.dueDate)}</span>
						)}
						{task.status === "doing" && (
							<span className="lc-task-active">Active</span>
						)}
					</button>
				))}
			</div>
		</div>
	);
}

function ScoreRing({ score, size = 44 }: { score: number; size?: number }) {
	const stroke = 3;
	const r = (size - stroke) / 2;
	const circumference = 2 * Math.PI * r;
	const offset = circumference - (Math.min(score, 100) / 100) * circumference;
	const color =
		score >= 70
			? "var(--success, #4ade80)"
			: score >= 40
				? "var(--brand, #6ea8fe)"
				: "var(--destructive, #f87171)";
	return (
		<svg
			width={size}
			height={size}
			className="lc-score-ring"
			role="img"
			aria-label={`Productivity score: ${score}`}
		>
			<circle
				cx={size / 2}
				cy={size / 2}
				r={r}
				fill="none"
				stroke="rgba(255,255,255,0.06)"
				strokeWidth={stroke}
			/>
			<circle
				cx={size / 2}
				cy={size / 2}
				r={r}
				fill="none"
				stroke={color}
				strokeWidth={stroke}
				strokeLinecap="round"
				strokeDasharray={circumference}
				strokeDashoffset={offset}
				style={{ transition: "stroke-dashoffset 0.8s ease-out" }}
			/>
			<text
				x={size / 2}
				y={size / 2}
				textAnchor="middle"
				dominantBaseline="central"
				fill="var(--text-primary)"
				fontSize="13"
				fontWeight="600"
				style={{ transform: "rotate(90deg)", transformOrigin: "center" }}
			>
				{score}
			</text>
		</svg>
	);
}

function ProductivityWidget({
	productivity,
}: {
	productivity: DashboardData["productivity"];
}) {
	const hours = Math.floor(productivity.totalMinutes / 60);
	const mins = productivity.totalMinutes % 60;
	const timeStr = hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
	return (
		<div className="lc-card">
			<div className="lc-prod-row">
				<ScoreRing score={productivity.score} />
				<div className="lc-prod-body">
					<div className="lc-prod-time">
						<span className="lc-prod-time-value">{timeStr}</span>
						<span className="lc-muted-xs">today</span>
					</div>
					<div className="lc-prod-cat">
						<div className="lc-prod-cat-row">
							<span className="lc-muted-xs">{productivity.topCategory}</span>
							<span className="lc-dim-xs">
								{Math.round(productivity.topCategoryPct)}%
							</span>
						</div>
						<div className="lc-prod-bar">
							<div
								className="lc-prod-bar-fill"
								style={{ width: `${productivity.topCategoryPct}%` }}
							/>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}
