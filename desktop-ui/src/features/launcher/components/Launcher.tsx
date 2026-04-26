import { useCallback, useEffect, useRef, useState } from "react";
import { useDashboardData } from "../hooks/useDashboardData";
import { useDndActive } from "../hooks/useDndActive";
import { executeItem } from "../hooks/useExecuteItem";
import { useKeyboardNavigation } from "../hooks/useKeyboardNavigation";
import { useLauncherSearch } from "../hooks/useLauncherSearch";
import { useTransparentBackground } from "../hooks/useTransparentBackground";
import { useWindowAutoResize } from "../hooks/useWindowAutoResize";
import {
	LauncherStoreProvider,
	useLauncherApi,
	useLauncherState,
} from "../store";
import { emit, getCurrentWindow, isTauri, listen } from "../tauri-bridge";
import { ActionMenu } from "./ActionMenu";
import { ArgChipBar } from "./ArgChipBar";
import { Dashboard } from "./Dashboard";
import { DetailPanel } from "./DetailPanel";
import { FocusActiveChip } from "./FocusActiveChip";
import { LauncherChatStub } from "./LauncherChatStub";
import { LauncherInput } from "./LauncherInput";
import { ResultsList } from "./ResultsList";
import { VoiceRecorderStub } from "./VoiceRecorderStub";

import "../launcher.css";

export function Launcher() {
	return (
		<LauncherStoreProvider>
			<LauncherShell />
		</LauncherStoreProvider>
	);
}

// Drag handle adds vertical chrome above the body; account for it when
// computing the total window height from the body's scrollHeight.
const DRAG_HANDLE_HEIGHT = 12;

function LauncherShell() {
	const contentRef = useRef<HTMLDivElement>(null);
	const bodyRef = useRef<HTMLDivElement>(null);
	const mode = useLauncherState((s) => s.mode);
	const argModeItem = useLauncherState((s) => s.argModeItem);
	const store = useLauncherApi();
	const { setMode, setArgModeItem, reset } = store;
	const dndActive = useDndActive();

	const [chatSessionKey, setChatSessionKey] = useState("");
	const [chatInitialQuery, setChatInitialQuery] = useState<string | null>(null);

	useTransparentBackground();
	useWindowAutoResize(bodyRef, {
		width: 660,
		minHeight: 360,
		maxHeight: 680,
		extra: DRAG_HANDLE_HEIGHT,
	});
	useLauncherSearch();
	useDashboardData();

	const hideWindow = useCallback(async () => {
		if (isTauri()) {
			try {
				await getCurrentWindow().hide();
			} catch {
				// silent
			}
		}
		reset();
	}, [reset]);

	const enterChat = useCallback(
		(query: string) => {
			setChatSessionKey(`launcher-${Date.now()}`);
			setChatInitialQuery(query);
			setMode("chat");
		},
		[setMode],
	);

	const cancelRecording = useCallback(async () => {
		reset();
		if (isTauri()) {
			try {
				await getCurrentWindow().hide();
			} catch {
				// silent
			}
		}
	}, [reset]);

	const expandToMain = useCallback(async () => {
		if (!isTauri()) return;
		try {
			await emit("navigate", { path: chatSessionKey ? "/chat" : "/" });
			if (chatSessionKey) {
				await emit("open-chat", { sessionKey: chatSessionKey });
			}
			await getCurrentWindow().hide();
		} catch {
			// silent
		}
		reset();
	}, [chatSessionKey, reset]);

	useEffect(() => {
		if (!isTauri()) return;
		const unlisteners: (() => void)[] = [];
		listen("voice-recording-start", () => {
			reset();
			setMode("recording");
		}).then((fn) => unlisteners.push(fn));
		listen("voice-recording-reset", () => {
			reset();
		}).then((fn) => unlisteners.push(fn));
		return () => {
			for (const fn of unlisteners) fn();
		};
	}, [setMode, reset]);

	useKeyboardNavigation({
		onEnterChat: enterChat,
		onExpandToMain: expandToMain,
		onHide: hideWindow,
	});

	const handleExecute = useCallback(
		(index: number) => {
			const results = store.getState().results;
			const item = results[index];
			if (!item) return;
			if (
				item.kind.type === "systemCommand" &&
				item.kind.action === "toggleDoNotDisturb" &&
				dndActive.data
			) {
				setArgModeItem(item);
				return;
			}
			executeItem(item, {
				store,
				onEnterChat: enterChat,
				onExpandToMain: expandToMain,
				onHide: hideWindow,
			});
		},
		[
			enterChat,
			expandToMain,
			hideWindow,
			dndActive.data,
			setArgModeItem,
			store,
		],
	);

	return (
		<div className="lc-root">
			<div ref={contentRef} className="lc-shell">
				<div
					className="lc-drag-handle"
					onMouseDown={async () => {
						if (!isTauri()) return;
						try {
							await getCurrentWindow().startDragging();
						} catch {
							// silent
						}
					}}
				>
					<div className="lc-drag-grip" />
				</div>
				{mode === "recording" ? (
					<VoiceRecorderStub
						onTranscriptReady={enterChat}
						onCancel={cancelRecording}
					/>
				) : mode === "chat" && chatSessionKey ? (
					<LauncherChatStub
						initialQuery={chatInitialQuery}
						onBack={() => {
							setMode("dashboard");
							reset();
						}}
						onExpand={expandToMain}
					/>
				) : (
					<div ref={bodyRef} className="lc-body">
						<LauncherInput />
						{argModeItem &&
						argModeItem.kind.type === "systemCommand" &&
						argModeItem.kind.action === "toggleDoNotDisturb" &&
						dndActive.data ? (
							<FocusActiveChip
								endsAt={dndActive.data.endsAt}
								onDone={() => setArgModeItem(null)}
							/>
						) : argModeItem ? (
							<ArgChipBar
								specs={argModeItem.arguments ?? []}
								onSubmit={(args) => {
									setArgModeItem(null);
									executeItem(
										argModeItem,
										{
											store,
											onEnterChat: enterChat,
											onExpandToMain: expandToMain,
											onHide: hideWindow,
										},
										args,
									);
								}}
								onCancel={() => setArgModeItem(null)}
							/>
						) : (
							<>
								{mode === "dashboard" && (
									<>
										<ShortcutHints />
										<Dashboard />
										<ResultsList onExecute={handleExecute} />
									</>
								)}
								{mode === "search" && <ResultsList onExecute={handleExecute} />}
								{mode === "detail" && <DetailPanel />}
							</>
						)}
						<ActionMenu />
					</div>
				)}
			</div>
		</div>
	);
}

function ShortcutHints() {
	const hints = [
		{ key: "f/", label: "Files" },
		{ key: "g/", label: "Grep" },
		{ key: "h/", label: "History" },
		{ key: "@", label: "Contacts" },
		{ key: ">", label: "Commands" },
		{ key: "?", label: "Ask AI" },
	];
	return (
		<div className="lc-hints">
			{hints.map((h) => (
				<span key={h.key} className="lc-hint">
					<kbd className="lc-kbd">{h.key}</kbd>
					{h.label}
				</span>
			))}
		</div>
	);
}
