// TODO(klynt-integration): aggregate mock surface for all Tauri sub-modules
// (api/app, api/dpi, api/menu, api/webview, api/window, plugin-dialog,
// plugin-notification, plugin-opener, plugin-process, plugin-updater,
// tauri-plugin-liquid-glass-api). vite.config.ts aliases each module name
// to a named export object below. To restore real Tauri APIs, delete the
// alias entries.

// ---- @tauri-apps/api/app ----
export async function getVersion(): Promise<string> {
	// TODO(klynt-integration): expose real app version once Tauri is wired.
	return "0.0.0-mock";
}

export async function getName(): Promise<string> {
	return "Klynt (mock)";
}

export async function getTauriVersion(): Promise<string> {
	return "0.0.0-mock";
}

// ---- @tauri-apps/api/dpi ----
export class LogicalPosition {
	constructor(
		public x: number,
		public y: number,
	) {}
}

// ---- @tauri-apps/api/menu ----
class MockMenuBase {
	static async new(_opts?: unknown): Promise<MockMenuBase> {
		return new MockMenuBase();
	}
	async setAsAppMenu(): Promise<void> {}
	async popup(): Promise<void> {}
	async append(_item: unknown): Promise<void> {}
	async remove(_item: unknown): Promise<void> {}
}
export class Menu extends MockMenuBase {}
export class MenuItem extends MockMenuBase {}
export class PredefinedMenuItem extends MockMenuBase {}

// ---- @tauri-apps/api/webview ----
// Proxy: explicit methods take precedence; any other method becomes a no-op
// async stub (resolves to undefined) so we don't have to enumerate Tauri's
// full API surface. TODO(klynt-integration): swap for the real webview handle.
function makeMockHandle(explicit: Record<string, unknown>) {
	return new Proxy(explicit, {
		get(target, prop) {
			if (prop in target) {
				return (target as Record<string | symbol, unknown>)[prop];
			}
			if (typeof prop === "symbol") {
				return undefined;
			}
			// Default: any unknown method returns a resolved promise.
			return (..._args: unknown[]) => Promise.resolve(undefined);
		},
	});
}

export function getCurrentWebview() {
	return makeMockHandle({
		label: "main",
		async onDragDropEvent(_handler: unknown): Promise<() => void> {
			return () => {};
		},
	});
}

// ---- @tauri-apps/api/window ----
export const Effect = {
	Mica: "mica",
	Tabbed: "tabbed",
	Vibrancy: "vibrancy",
	Blur: "blur",
	AcrylicMaterial: "acrylicMaterial",
	HudWindow: "hudWindow",
	FullScreenUI: "fullScreenUI",
	UnderWindowBackground: "underWindowBackground",
	UnderPageBackground: "underPageBackground",
} as const;

export const EffectState = {
	FollowsWindowActiveState: "followsWindowActiveState",
	Active: "active",
	Inactive: "inactive",
} as const;

export function getCurrentWindow() {
	return makeMockHandle({
		label: "main",
		async setTitle(_t: string): Promise<void> {},
		async setSize(_s: unknown): Promise<void> {},
		async setMinSize(_s: unknown): Promise<void> {},
		async maximize(): Promise<void> {},
		async unmaximize(): Promise<void> {},
		async minimize(): Promise<void> {},
		async toggleMaximize(): Promise<void> {},
		async close(): Promise<void> {},
		async hide(): Promise<void> {},
		async show(): Promise<void> {},
		async startDragging(): Promise<void> {},
		async setFocus(): Promise<void> {},
		async isMaximized(): Promise<boolean> {
			return false;
		},
		async isMinimized(): Promise<boolean> {
			return false;
		},
		async isFocused(): Promise<boolean> {
			return true;
		},
		async isFullscreen(): Promise<boolean> {
			return false;
		},
		async setFullscreen(_v: boolean): Promise<void> {},
		async setEffects(_e: unknown): Promise<void> {},
		async clearEffects(): Promise<void> {},
		async onResized(_cb: unknown): Promise<() => void> {
			return () => {};
		},
		async onFocusChanged(_cb: unknown): Promise<() => void> {
			return () => {};
		},
		async onCloseRequested(_cb: unknown): Promise<() => void> {
			return () => {};
		},
		async onScaleChanged(_cb: unknown): Promise<() => void> {
			return () => {};
		},
		async listen(_event: string, _cb: unknown): Promise<() => void> {
			return () => {};
		},
	});
}

// ---- @tauri-apps/plugin-dialog ----
export async function open(_opts?: unknown): Promise<string | string[] | null> {
	// TODO(klynt-integration): wire to native file picker.
	console.debug("[mock dialog.open]", _opts);
	return null;
}

export async function save(_opts?: unknown): Promise<string | null> {
	console.debug("[mock dialog.save]", _opts);
	return null;
}

export async function ask(message: string, _opts?: unknown): Promise<boolean> {
	console.debug("[mock dialog.ask]", message);
	return false;
}

export async function message(msg: string, _opts?: unknown): Promise<void> {
	console.debug("[mock dialog.message]", msg);
}

// ---- @tauri-apps/plugin-notification ----
export type NotificationOptions = {
	title: string;
	body?: string;
	icon?: string;
	sound?: string;
};

export async function isPermissionGranted(): Promise<boolean> {
	return false;
}

export async function requestPermission(): Promise<
	"granted" | "denied" | "default"
> {
	return "denied";
}

export function sendNotification(opts: NotificationOptions | string): void {
	console.debug("[mock sendNotification]", opts);
}

// Namespace import support for `import * as notification from "@tauri-apps/plugin-notification"`
export const notificationNamespace = {
	isPermissionGranted,
	requestPermission,
	sendNotification,
};

// ---- @tauri-apps/plugin-opener ----
export async function openUrl(url: string): Promise<void> {
	console.debug("[mock openUrl]", url);
	if (typeof window !== "undefined") {
		window.open(url, "_blank", "noopener,noreferrer");
	}
}

export async function revealItemInDir(path: string): Promise<void> {
	console.debug("[mock revealItemInDir]", path);
}

// ---- @tauri-apps/plugin-process ----
export async function relaunch(): Promise<void> {
	console.debug("[mock relaunch]");
	if (typeof window !== "undefined") {
		window.location.reload();
	}
}

export async function exit(code = 0): Promise<void> {
	console.debug("[mock exit]", code);
}

// ---- @tauri-apps/plugin-updater ----
export type DownloadEvent =
	| { event: "Started"; data: { contentLength?: number } }
	| { event: "Progress"; data: { chunkLength: number } }
	| { event: "Finished" };

export type Update = {
	version: string;
	date?: string;
	body?: string;
	available: boolean;
	downloadAndInstall: (cb?: (e: DownloadEvent) => void) => Promise<void>;
	download: (cb?: (e: DownloadEvent) => void) => Promise<void>;
	install: () => Promise<void>;
};

export async function check(): Promise<Update | null> {
	// TODO(klynt-integration): wire to real updater. Returning null = "no update".
	return null;
}

// ---- tauri-plugin-liquid-glass-api ----
export const GlassMaterialVariant = {
	Regular: "regular",
	Thin: "thin",
	Thick: "thick",
	UltraThin: "ultraThin",
	UltraThick: "ultraThick",
	Chrome: "chrome",
} as const;
export type GlassMaterialVariant =
	(typeof GlassMaterialVariant)[keyof typeof GlassMaterialVariant];

export async function isGlassSupported(): Promise<boolean> {
	return false;
}

export async function setLiquidGlassEffect(_opts?: unknown): Promise<void> {
	console.debug("[mock setLiquidGlassEffect]", _opts);
}
