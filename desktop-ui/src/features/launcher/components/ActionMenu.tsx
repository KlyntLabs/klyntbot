import { useCallback, useEffect, useRef, useState } from "react";
import { useLauncherApi, useLauncherState } from "../store";
import { ipc } from "@/utils/tauri-bridge";
import type { LauncherItem, LauncherItemKind } from "../types";

interface Action {
	label: string;
	shortcut?: string;
	handler: () => void;
}

export function ActionMenu() {
	const actionMenuOpen = useLauncherState((s) => s.actionMenuOpen);
	const results = useLauncherState((s) => s.results);
	const selectedIndex = useLauncherState((s) => s.selectedIndex);
	const { setActionMenuOpen } = useLauncherApi();
	const [focusedIndex, setFocusedIndex] = useState(0);
	const menuRef = useRef<HTMLDivElement>(null);

	const item = results[selectedIndex] ?? null;
	const actions = item ? getActionsForItem(item) : [];

	const close = useCallback(
		() => setActionMenuOpen(false),
		[setActionMenuOpen],
	);

	const executeAction = useCallback(
		(index: number) => {
			const action = actions[index];
			if (action) {
				action.handler();
				close();
			}
		},
		[actions, close],
	);

	useEffect(() => {
		if (actionMenuOpen) setFocusedIndex(0);
	}, [actionMenuOpen]);

	useEffect(() => {
		if (!actionMenuOpen) return;
		const handleKeyDown = (e: KeyboardEvent) => {
			switch (e.key) {
				case "Escape":
					e.preventDefault();
					e.stopPropagation();
					close();
					break;
				case "ArrowDown":
					e.preventDefault();
					e.stopPropagation();
					setFocusedIndex((p) => Math.min(p + 1, actions.length - 1));
					break;
				case "ArrowUp":
					e.preventDefault();
					e.stopPropagation();
					setFocusedIndex((p) => Math.max(p - 1, 0));
					break;
				case "Enter":
					e.preventDefault();
					e.stopPropagation();
					executeAction(focusedIndex);
					break;
				default: {
					const num = Number.parseInt(e.key, 10);
					if (num >= 1 && num <= 9 && num <= actions.length) {
						e.preventDefault();
						e.stopPropagation();
						executeAction(num - 1);
					}
					break;
				}
			}
		};
		window.addEventListener("keydown", handleKeyDown, true);
		return () => window.removeEventListener("keydown", handleKeyDown, true);
	}, [actionMenuOpen, actions.length, close, executeAction, focusedIndex]);

	if (!actionMenuOpen || !item) return null;

	return (
		<div
			role="dialog"
			className="lc-action-overlay"
			onClick={close}
			onKeyDown={() => {}}
		>
			<div className="lc-action-backdrop" />
			<div
				ref={menuRef}
				role="menu"
				className="lc-action-menu"
				onClick={(e) => e.stopPropagation()}
				onKeyDown={() => {}}
			>
				<div className="lc-action-header">
					<div className="lc-action-eyebrow">Actions</div>
					<div className="lc-action-target">{item.title}</div>
				</div>
				<div className="lc-action-list">
					{actions.map((action, index) => (
						<button
							key={action.label}
							type="button"
							className={`lc-action-item${index === focusedIndex ? " is-focused" : ""}`}
							onClick={() => executeAction(index)}
							onMouseEnter={() => setFocusedIndex(index)}
						>
							<span className="lc-action-num">{index + 1}</span>
							<span className="lc-action-label">{action.label}</span>
							{action.shortcut && (
								<span className="lc-action-shortcut">{action.shortcut}</span>
							)}
						</button>
					))}
				</div>
				<div className="lc-action-footer">
					1-9 quick select · ↑↓ navigate · Esc close
				</div>
			</div>
		</div>
	);
}

function copyText(text: string) {
	navigator.clipboard
		.writeText(text)
		.catch((err) => console.error("Failed to copy:", err));
}

function openPath(path: string) {
	ipc("launcher_open_app", { path }).catch((err) =>
		console.error("Failed to open:", err),
	);
}

function getActionsForItem(item: LauncherItem): Action[] {
	const { kind } = item;
	switch (kind.type) {
		case "application":
			return applicationActions(item, kind);
		case "file":
			return fileActions(kind);
		case "contentMatch":
			return contentMatchActions(kind);
		case "gitRepo":
			return gitRepoActions(kind);
		case "bookmark":
		case "browserHistory":
		case "urlNavigation":
			return urlActions(kind as { url: string });
		case "task":
			return taskActions(item);
		case "note":
			return noteActions(item);
		case "calculator":
			return calculatorActions(kind);
		case "contact":
			return contactActions(kind);
		case "systemCommand":
		case "script":
			return executeActions(item);
		case "sshHost":
			return sshActions(kind);
		case "brewPackage":
			return brewActions(kind);
		default:
			return defaultActions(item);
	}
}

type KindOf<T extends LauncherItemKind["type"]> = Extract<
	LauncherItemKind,
	{ type: T }
>;

function applicationActions(
	item: LauncherItem,
	kind: KindOf<"application">,
): Action[] {
	return [
		{ label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
		{
			label: "Show in Finder",
			handler: () => openPath(kind.path.replace(/\/[^/]+$/, "")),
		},
		{ label: "Copy Path", handler: () => copyText(kind.path) },
		{ label: "Copy Name", handler: () => copyText(item.title) },
	];
}

function fileActions(kind: KindOf<"file">): Action[] {
	const actions: Action[] = [
		{ label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
		{
			label: "Reveal in Finder",
			handler: () => openPath(kind.path.replace(/\/[^/]+$/, "")),
		},
		{ label: "Copy Path", handler: () => copyText(kind.path) },
	];
	if (kind.kind === "folder") {
		actions.push({
			label: "Open in Terminal",
			handler: () => openPath(`terminal://${kind.path}`),
		});
	}
	return actions;
}

function contentMatchActions(kind: KindOf<"contentMatch">): Action[] {
	return [
		{ label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
		{
			label: "Reveal in Finder",
			handler: () => openPath(kind.path.replace(/\/[^/]+$/, "")),
		},
		{ label: "Copy Path", handler: () => copyText(kind.path) },
	];
}

function gitRepoActions(kind: KindOf<"gitRepo">): Action[] {
	return [
		{ label: "Open", shortcut: "Enter", handler: () => openPath(kind.path) },
		{ label: "Reveal in Finder", handler: () => openPath(kind.path) },
		{ label: "Copy Path", handler: () => copyText(kind.path) },
		{
			label: "Open in Terminal",
			handler: () => openPath(`terminal://${kind.path}`),
		},
	];
}

function urlActions(kind: { url: string }): Action[] {
	return [
		{ label: "Open URL", shortcut: "Enter", handler: () => openPath(kind.url) },
		{ label: "Copy URL", handler: () => copyText(kind.url) },
	];
}

function taskActions(item: LauncherItem): Action[] {
	return [
		{
			label: "Open in App",
			shortcut: "Enter",
			handler: () => openPath(`klyntbot://task/${item.id}`),
		},
		{ label: "Copy Title", handler: () => copyText(item.title) },
	];
}

function noteActions(item: LauncherItem): Action[] {
	return [
		{
			label: "Open in App",
			shortcut: "Enter",
			handler: () => openPath(`klyntbot://note/${item.id}`),
		},
		{ label: "Copy Title", handler: () => copyText(item.title) },
	];
}

function calculatorActions(kind: KindOf<"calculator">): Action[] {
	return [
		{
			label: "Copy Result",
			shortcut: "Enter",
			handler: () => copyText(String(kind.result)),
		},
		{ label: "Copy Expression", handler: () => copyText(kind.expression) },
	];
}

function contactActions(kind: KindOf<"contact">): Action[] {
	const actions: Action[] = [];
	const { email, phone } = kind;
	if (email)
		actions.push({
			label: "Email",
			shortcut: "Enter",
			handler: () => openPath(`mailto:${email}`),
		});
	if (phone)
		actions.push({ label: "Call", handler: () => openPath(`tel:${phone}`) });
	if (email)
		actions.push({ label: "Copy Email", handler: () => copyText(email) });
	if (phone)
		actions.push({ label: "Copy Phone", handler: () => copyText(phone) });
	return actions;
}

function executeActions(item: LauncherItem): Action[] {
	return [
		{ label: "Execute", shortcut: "Enter", handler: () => {} },
		{ label: "Copy Title", handler: () => copyText(item.title) },
	];
}

function sshActions(kind: KindOf<"sshHost">): Action[] {
	const sshCmd = kind.user
		? `ssh ${kind.user}@${kind.host}`
		: `ssh ${kind.host}`;
	const sshUri = kind.user
		? `ssh://${kind.user}@${kind.host}`
		: `ssh://${kind.host}`;
	return [
		{ label: "Connect", shortcut: "Enter", handler: () => openPath(sshUri) },
		{ label: "Copy SSH Command", handler: () => copyText(sshCmd) },
	];
}

function brewActions(kind: KindOf<"brewPackage">): Action[] {
	const installCmd = kind.isCask
		? `brew install --cask ${kind.name}`
		: `brew install ${kind.name}`;
	return [
		{
			label: "Copy Name",
			shortcut: "Enter",
			handler: () => copyText(kind.name),
		},
		{ label: "Copy Install Command", handler: () => copyText(installCmd) },
	];
}

function defaultActions(item: LauncherItem): Action[] {
	return [
		{ label: "Execute", shortcut: "Enter", handler: () => {} },
		{ label: "Copy Title", handler: () => copyText(item.title) },
	];
}
