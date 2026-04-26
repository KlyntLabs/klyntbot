import type { ReactNode } from "react";
import { createContext, useContext, useMemo, useReducer, useRef } from "react";
import type { DashboardData, LauncherItem, LauncherMode } from "./types";

interface State {
	mode: LauncherMode;
	query: string;
	results: LauncherItem[];
	selectedIndex: number;
	isSearching: boolean;
	dashboard: DashboardData | null;
	queryHistory: string[];
	historyIndex: number;
	detailItem: LauncherItem | null;
	actionMenuOpen: boolean;
	argModeItem: LauncherItem | null;
}

const initial: State = {
	mode: "dashboard",
	query: "",
	results: [],
	selectedIndex: 0,
	isSearching: false,
	dashboard: null,
	queryHistory: [],
	historyIndex: -1,
	detailItem: null,
	actionMenuOpen: false,
	argModeItem: null,
};

type Action =
	| { type: "patch"; patch: Partial<State> }
	| { type: "setQuery"; query: string }
	| { type: "moveSelection"; delta: number }
	| { type: "pushHistory"; query: string }
	| { type: "navigateHistory"; direction: "up" | "down" }
	| { type: "reset" };

function reducer(s: State, a: Action): State {
	switch (a.type) {
		case "patch":
			return { ...s, ...a.patch };
		case "setQuery": {
			const mode: LauncherMode = a.query.length > 0 ? "search" : "dashboard";
			return { ...s, query: a.query, mode, selectedIndex: 0, historyIndex: -1 };
		}
		case "moveSelection": {
			const next = Math.max(
				0,
				Math.min(s.results.length - 1, s.selectedIndex + a.delta),
			);
			return { ...s, selectedIndex: next };
		}
		case "pushHistory": {
			if (!a.query.trim()) return s;
			const filtered = s.queryHistory.filter((q) => q !== a.query);
			return { ...s, queryHistory: [a.query, ...filtered].slice(0, 50) };
		}
		case "navigateHistory": {
			if (s.queryHistory.length === 0) return s;
			const next =
				a.direction === "up"
					? Math.min(s.historyIndex + 1, s.queryHistory.length - 1)
					: Math.max(-1, s.historyIndex - 1);
			if (next === -1) return { ...s, historyIndex: -1, query: "" };
			return { ...s, historyIndex: next, query: s.queryHistory[next] };
		}
		case "reset":
			return { ...initial, queryHistory: s.queryHistory };
	}
}

export interface LauncherStoreApi {
	setMode: (mode: LauncherMode) => void;
	setQuery: (q: string) => void;
	setResults: (r: LauncherItem[]) => void;
	setSelectedIndex: (i: number) => void;
	setIsSearching: (v: boolean) => void;
	setDashboard: (d: DashboardData) => void;
	moveSelection: (delta: number) => void;
	pushHistory: (q: string) => void;
	navigateHistory: (direction: "up" | "down") => void;
	setDetailItem: (item: LauncherItem | null) => void;
	setActionMenuOpen: (open: boolean) => void;
	setArgModeItem: (item: LauncherItem | null) => void;
	reset: () => void;
	getState: () => State;
}

const StateCtx = createContext<State | null>(null);
const ApiCtx = createContext<LauncherStoreApi | null>(null);

export function LauncherStoreProvider({ children }: { children: ReactNode }) {
	const [state, dispatch] = useReducer(reducer, initial);
	const stateRef = useRef(state);
	stateRef.current = state;

	const api = useMemo<LauncherStoreApi>(
		() => ({
			setMode: (mode) => dispatch({ type: "patch", patch: { mode } }),
			setQuery: (query) => dispatch({ type: "setQuery", query }),
			setResults: (results) =>
				dispatch({ type: "patch", patch: { results, isSearching: false } }),
			setSelectedIndex: (selectedIndex) =>
				dispatch({ type: "patch", patch: { selectedIndex } }),
			setIsSearching: (isSearching) =>
				dispatch({ type: "patch", patch: { isSearching } }),
			setDashboard: (dashboard) =>
				dispatch({ type: "patch", patch: { dashboard } }),
			moveSelection: (delta) => dispatch({ type: "moveSelection", delta }),
			pushHistory: (query) => dispatch({ type: "pushHistory", query }),
			navigateHistory: (direction) =>
				dispatch({ type: "navigateHistory", direction }),
			setDetailItem: (detailItem) =>
				dispatch({ type: "patch", patch: { detailItem } }),
			setActionMenuOpen: (actionMenuOpen) =>
				dispatch({ type: "patch", patch: { actionMenuOpen } }),
			setArgModeItem: (argModeItem) =>
				dispatch({ type: "patch", patch: { argModeItem } }),
			reset: () => dispatch({ type: "reset" }),
			getState: () => stateRef.current,
		}),
		[],
	);

	return (
		<StateCtx.Provider value={state}>
			<ApiCtx.Provider value={api}>{children}</ApiCtx.Provider>
		</StateCtx.Provider>
	);
}

export function useLauncherState<T>(selector: (s: State) => T): T {
	const s = useContext(StateCtx);
	if (!s) throw new Error("LauncherStoreProvider missing");
	return selector(s);
}

export function useLauncherApi(): LauncherStoreApi {
	const a = useContext(ApiCtx);
	if (!a) throw new Error("LauncherStoreProvider missing");
	return a;
}

// Combined-shape selector to mimic the .bak's `useLauncherStore((s) => s.x)` ergonomics
// where `x` could be either state or an action. Components destructure either context.
export function useLauncher<T>(
	selector: (s: State & LauncherStoreApi) => T,
): T {
	const state = useContext(StateCtx);
	const api = useContext(ApiCtx);
	if (!state || !api) throw new Error("LauncherStoreProvider missing");
	const merged = useMemo(
		() => ({ ...state, ...api }) as State & LauncherStoreApi,
		[state, api],
	);
	return selector(merged);
}
