// Direct bridge to Tauri 2 internals, bypassing the project-wide
// `@tauri-apps/api/*` Vite mocks. The launcher is one of the few features
// that genuinely needs the real Tauri runtime.

type Internals = {
	invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
	transformCallback: (
		handler: (event: unknown) => void,
		once?: boolean,
	) => number;
	convertFileSrc?: (filePath: string, protocol?: string) => string;
};

function internals(): Internals | null {
	if (typeof window === "undefined") return null;
	const w = window as unknown as { __TAURI_INTERNALS__?: Internals };
	return w.__TAURI_INTERNALS__ ?? null;
}

export const isTauri = (): boolean => internals() !== null;

export async function invoke<T = unknown>(
	cmd: string,
	args?: Record<string, unknown>,
): Promise<T> {
	const i = internals();
	if (!i) throw new Error(`Tauri not available: cannot invoke '${cmd}'`);
	return i.invoke<T>(cmd, args);
}

// `ipc` mirrors the .bak helper signature used throughout the launcher.
export const ipc = invoke;

export function convertFileSrc(filePath: string, protocol = "asset"): string {
	const i = internals();
	if (!i || !i.convertFileSrc) {
		// Fallback used by webview asset protocol on macOS (Tauri 2 default).
		return `${protocol}://localhost/${encodeURIComponent(filePath)}`;
	}
	return i.convertFileSrc(filePath, protocol);
}

type EventPayload<T> = { event: string; id: number; payload: T };

// Replicates @tauri-apps/api/event#listen for the current window only.
export async function listen<T = unknown>(
	event: string,
	handler: (payload: T) => void,
): Promise<() => void> {
	const i = internals();
	if (!i) return () => {};
	const cb = i.transformCallback((evt) => {
		const e = evt as EventPayload<T>;
		handler(e.payload);
	}, false);
	const eventId = await i.invoke<number>("plugin:event|listen", {
		event,
		target: { kind: "Any" },
		handler: cb,
	});
	return async () => {
		try {
			await i.invoke("plugin:event|unlisten", { event, eventId });
		} catch {
			// already unlistened
		}
	};
}

export async function emit(event: string, payload?: unknown): Promise<void> {
	const i = internals();
	if (!i) return;
	await i.invoke("plugin:event|emit", { event, payload });
}

export interface CurrentWindow {
	label: string;
	hide(): Promise<void>;
	show(): Promise<void>;
	setFocus(): Promise<void>;
	startDragging(): Promise<void>;
	setSize(width: number, height: number): Promise<void>;
	onFocusChanged(
		handler: (e: { payload: boolean }) => void,
	): Promise<() => void>;
	listen<T = unknown>(
		event: string,
		handler: (payload: T) => void,
	): Promise<() => void>;
}

// Read the canonical Tauri 2 window label. Lives under
// `window.__TAURI_INTERNALS__.metadata.currentWindow.label` — same path the
// real `@tauri-apps/api/window#getCurrentWindow()` uses internally.
export function currentWindowLabel(): string {
	if (typeof window === "undefined") return "main";
	const w = window as unknown as {
		__TAURI_INTERNALS__?: {
			metadata?: { currentWindow?: { label: string } };
		};
	};
	const label = w.__TAURI_INTERNALS__?.metadata?.currentWindow?.label;
	if (label) return label;
	// Browser-only dev fallback so `localhost:1420/#/launcher` still routes.
	if (
		typeof window.location !== "undefined" &&
		window.location.hash === "#/launcher"
	) {
		return "launcher";
	}
	return "main";
}

export function getCurrentWindow(): CurrentWindow {
	const label = currentWindowLabel();
	return {
		label,
		hide: () => invoke<void>("plugin:window|hide"),
		show: () => invoke<void>("plugin:window|show"),
		setFocus: () => invoke<void>("plugin:window|set_focus"),
		startDragging: () => invoke<void>("plugin:window|start_dragging"),
		// Wire format matches what `Size.toJSON()` in @tauri-apps/api v2
		// produces: untagged `{ Logical: { width, height } }`. The command
		// parameter is named `value` and we pass `label` so the call targets
		// the launcher window explicitly.
		setSize: (width, height) =>
			invoke<void>("plugin:window|set_size", {
				label,
				value: { Logical: { width, height } },
			}),
		onFocusChanged: async (handler) => {
			const offFocus = await listen<null>("tauri://focus", () =>
				handler({ payload: true }),
			);
			const offBlur = await listen<null>("tauri://blur", () =>
				handler({ payload: false }),
			);
			return () => {
				offFocus();
				offBlur();
			};
		},
		listen: (event, handler) => listen(event, handler),
	};
}
