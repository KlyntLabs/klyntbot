import { useEffect, useRef } from "react";
import { useLauncherApi, useLauncherState } from "../store";
import { getCurrentWindow, isTauri } from "@/utils/tauri-bridge";

export function LauncherInput() {
	const inputRef = useRef<HTMLInputElement>(null);
	const query = useLauncherState((s) => s.query);
	const isSearching = useLauncherState((s) => s.isSearching);
	const { setQuery } = useLauncherApi();

	useEffect(() => {
		inputRef.current?.focus();
	}, []);

	useEffect(() => {
		if (!isTauri()) return;
		const unlisteners: (() => void)[] = [];
		const focusInput = () => setTimeout(() => inputRef.current?.focus(), 50);
		const win = getCurrentWindow();
		win
			.onFocusChanged(({ payload: focused }) => {
				if (focused) focusInput();
			})
			.then((fn) => unlisteners.push(fn));
		win
			.listen("window-shown", () => focusInput())
			.then((fn) => unlisteners.push(fn));
		return () => {
			for (const fn of unlisteners) fn();
		};
	}, []);

	return (
		<div className="lc-input-row">
			{isSearching ? (
				<div className="lc-input-spinner">
					<span />
					<span />
					<span />
				</div>
			) : (
				<svg
					className="lc-input-icon"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					strokeWidth={2}
					aria-hidden="true"
					role="img"
				>
					<title>Search</title>
					<path
						strokeLinecap="round"
						strokeLinejoin="round"
						d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
					/>
				</svg>
			)}
			<input
				ref={inputRef}
				type="text"
				value={query}
				onChange={(e) => setQuery(e.target.value)}
				placeholder="Search apps, tasks, notes, or ask AI..."
				className="lc-input-field"
				spellCheck={false}
				autoComplete="off"
			/>
			{query && (
				<button
					type="button"
					onClick={() => setQuery("")}
					className="lc-input-clear"
				>
					ESC
				</button>
			)}
		</div>
	);
}
