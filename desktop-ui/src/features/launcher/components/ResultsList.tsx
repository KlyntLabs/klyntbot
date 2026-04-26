import { useEffect, useRef } from "react";
import { useDndActive } from "../hooks/useDndActive";
import { formatRemaining } from "../lib/formatRemaining";
import { useLauncherApi, useLauncherState } from "../store";
import { convertFileSrc, isTauri } from "../tauri-bridge";
import type { FocusSession, LauncherItem } from "../types";

interface ResultsListProps {
	onExecute: (index: number) => void;
}

export function ResultsList({ onExecute }: ResultsListProps) {
	const results = useLauncherState((s) => s.results);
	const selectedIndex = useLauncherState((s) => s.selectedIndex);
	const isSearching = useLauncherState((s) => s.isSearching);
	const dndActive = useDndActive();
	const { setSelectedIndex } = useLauncherApi();
	const listRef = useRef<HTMLDivElement>(null);
	const mouseMovedRef = useRef(false);
	const lastMousePosRef = useRef<{ x: number; y: number } | null>(null);

	const resultsKey = results.length > 0 ? results[0].id : "";
	useEffect(() => {
		mouseMovedRef.current = false;
		lastMousePosRef.current = null;
	}, [resultsKey]);

	useEffect(() => {
		const list = listRef.current;
		if (!list) return;
		const selected = list.children[selectedIndex] as HTMLElement | undefined;
		selected?.scrollIntoView({ block: "nearest" });
	}, [selectedIndex]);

	if (isSearching && results.length === 0) {
		return (
			<div className="lc-empty">
				<div className="lc-empty-dots">
					<span style={{ animationDelay: "0ms" }} />
					<span style={{ animationDelay: "150ms" }} />
					<span style={{ animationDelay: "300ms" }} />
				</div>
				<span className="lc-muted-sm">Searching...</span>
			</div>
		);
	}

	if (results.length === 0) {
		return <div className="lc-empty-text">No results</div>;
	}

	const uniqueGroups = new Set(
		results.map((item) => groupLabel(item.kind.type)),
	);
	const showHeaders = uniqueGroups.size >= 2;
	let lastGroup = "";

	return (
		<div
			ref={listRef}
			role="listbox"
			className="lc-results"
			onMouseMove={(e) => {
				const last = lastMousePosRef.current;
				if (
					last &&
					(Math.abs(e.clientX - last.x) > 3 || Math.abs(e.clientY - last.y) > 3)
				) {
					mouseMovedRef.current = true;
				}
				lastMousePosRef.current = { x: e.clientX, y: e.clientY };
			}}
		>
			{results.map((item, index) => {
				const group = groupLabel(item.kind.type);
				const showHeader = showHeaders && group !== "" && group !== lastGroup;
				lastGroup = group;
				return (
					<div key={item.id}>
						{showHeader && <div className="lc-group-header">{group}</div>}
						<ResultRow
							item={item}
							index={index}
							isSelected={index === selectedIndex}
							dndSession={dndActive.data}
							onClick={() => onExecute(index)}
							onMouseEnter={() => {
								if (mouseMovedRef.current) setSelectedIndex(index);
							}}
						/>
					</div>
				);
			})}
		</div>
	);
}

function groupLabel(kind: string): string {
	if (kind === "application" || kind === "runningApp") return "Apps";
	if (kind === "file" || kind === "contentMatch" || kind === "gitRepo")
		return "Files";
	if (kind === "task") return "Tasks";
	if (kind === "note") return "Notes";
	if (
		kind === "bookmark" ||
		kind === "browserHistory" ||
		kind === "urlNavigation"
	)
		return "Web";
	return "";
}

function ResultRow({
	item,
	index,
	isSelected,
	dndSession,
	onClick,
	onMouseEnter,
}: {
	item: LauncherItem;
	index: number;
	isSelected: boolean;
	dndSession: FocusSession | null;
	onClick: () => void;
	onMouseEnter: () => void;
}) {
	const isDndItem =
		item.kind.type === "systemCommand" &&
		item.kind.action === "toggleDoNotDisturb";
	const displayTitle =
		isDndItem && dndSession
			? `DND on — ${formatRemaining(dndSession.endsAt)} left`
			: item.title;

	return (
		<div
			role="option"
			tabIndex={-1}
			aria-selected={isSelected}
			className={`lc-row${isSelected ? " is-selected" : ""}`}
			style={{ animationDelay: `${Math.min(index * 10, 100)}ms` }}
			onClick={onClick}
			onKeyDown={(e) => {
				if (e.key === "Enter" || e.key === " ") {
					e.preventDefault();
					onClick();
				}
			}}
			onMouseEnter={onMouseEnter}
		>
			<ItemIcon kind={item.kind.type} icon={item.icon} />
			<div className="lc-row-text">
				<div className="lc-row-title">{displayTitle}</div>
				{item.subtitle && (
					<div className="lc-row-subtitle">{item.subtitle}</div>
				)}
			</div>
			<KindBadge type={item.kind.type} />
		</div>
	);
}

const ICON_MAP: Record<string, string> = {
	application: "🫟",
	task: "✓",
	note: "📝",
	clipboardEntry: "📋",
	systemCommand: "⚙️",
	script: "▶",
	calculator: "🔢",
	calendar: "📅",
	aiChat: "✨",
	file: "📄",
	contentMatch: "🔍",
	contact: "👤",
	systemPref: "⚙️",
	runningApp: "🟢",
	bookmark: "🔖",
	browserHistory: "🌐",
	brewPackage: "🍺",
	sshHost: "🔑",
	gitRepo: "📂",
	urlNavigation: "🌐",
};

function ItemIcon({ kind, icon }: { kind: string; icon?: string | null }) {
	if (icon?.startsWith("/")) {
		const src = isTauri() ? convertFileSrc(icon) : icon;
		return <img src={src} alt="" className="lc-row-img" loading="lazy" />;
	}
	if (icon?.startsWith("data:")) {
		return <img src={icon} alt="" className="lc-row-img" loading="lazy" />;
	}
	return <span className="lc-row-emoji">{ICON_MAP[kind] || "•"}</span>;
}

const KIND_LABELS: Record<string, string> = {
	application: "App",
	task: "Task",
	note: "Note",
	clipboardEntry: "Clip",
	systemCommand: "Cmd",
	script: "Script",
	calculator: "Calc",
	calendar: "Event",
	aiChat: "AI",
	file: "File",
	contentMatch: "Match",
	contact: "Contact",
	systemPref: "Pref",
	runningApp: "Running",
	bookmark: "Bookmark",
	browserHistory: "History",
	brewPackage: "Brew",
	sshHost: "SSH",
	gitRepo: "Repo",
	urlNavigation: "URL",
};

function KindBadge({ type }: { type: string }) {
	return <span className="lc-row-badge">{KIND_LABELS[type] || type}</span>;
}
