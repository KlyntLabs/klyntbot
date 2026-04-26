import { useEffect, useRef } from "react";
import { useLauncherApi, useLauncherState } from "../store";
import { ipc } from "@/utils/tauri-bridge";
import type { LauncherItem } from "../types";

export function useLauncherSearch() {
	const query = useLauncherState((s) => s.query);
	const { setResults, setIsSearching } = useLauncherApi();
	const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	const versionRef = useRef(0);

	useEffect(() => {
		ipc<LauncherItem[]>("launcher_search", { query: "" })
			.then(setResults)
			.catch(() => {});
	}, [setResults]);

	useEffect(() => {
		if (!query.trim()) {
			const v = ++versionRef.current;
			ipc<LauncherItem[]>("launcher_search", { query: "" })
				.then((results) => {
					if (versionRef.current === v) setResults(results);
				})
				.catch(() => {});
			return;
		}

		setIsSearching(true);
		clearTimeout(timerRef.current);
		const version = ++versionRef.current;

		timerRef.current = setTimeout(async () => {
			try {
				const results = await ipc<LauncherItem[]>("launcher_search", { query });
				if (versionRef.current === version) {
					setResults(results);
					setIsSearching(false);
				}
			} catch (e) {
				if (versionRef.current === version) {
					console.error("Launcher search failed:", e);
					setResults([]);
					setIsSearching(false);
				}
			}
		}, 30);

		return () => clearTimeout(timerRef.current);
	}, [query, setResults, setIsSearching]);
}
