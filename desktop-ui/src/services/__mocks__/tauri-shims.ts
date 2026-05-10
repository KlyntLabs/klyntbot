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
//
// Browser-mode fallback: in dev mode there's no native picker (Chrome's
// File System Access API can't return absolute paths, which the Rust backend
// requires). We fall back to `window.prompt`/`confirm`/`alert` so the
// workspace lifecycle is still drivable end-to-end. Real Tauri builds replace
// these via the vite alias being stripped at build time.
type DialogOpenOptions = {
  directory?: boolean;
  multiple?: boolean;
  title?: string;
  defaultPath?: string;
};
type DialogSaveOptions = {
  title?: string;
  defaultPath?: string;
};
type DialogPromptOptions = {
  title?: string;
  kind?: "info" | "warning" | "error";
  okLabel?: string;
  cancelLabel?: string;
};

export async function open(opts: DialogOpenOptions = {}): Promise<string | string[] | null> {
  if (typeof window === "undefined") return null;
  const directory = Boolean(opts.directory);
  const multiple = Boolean(opts.multiple);
  const title = opts.title ?? (directory ? "Select folder" : "Select file");
  const placeholder = directory ? "/path/to/folder" : "/path/to/file";
  const hint = multiple
    ? `${title}\nEnter one or more absolute ${directory ? "folders" : "files"} (comma-separated).`
    : `${title}\n${placeholder}`;
  const raw = window.prompt(hint, opts.defaultPath ?? "");
  if (raw == null) return null;
  const trimmed = raw.trim();
  if (trimmed === "") return null;
  if (!multiple) return trimmed;
  const parts = trimmed
    .split(",")
    .map((p) => p.trim())
    .filter((p) => p.length > 0);
  return parts.length === 0 ? null : parts;
}

export async function save(opts: DialogSaveOptions = {}): Promise<string | null> {
  if (typeof window === "undefined") return null;
  const title = opts.title ?? "Save file";
  const raw = window.prompt(`${title}\n/path/to/file`, opts.defaultPath ?? "");
  if (raw == null) return null;
  const trimmed = raw.trim();
  return trimmed === "" ? null : trimmed;
}

export async function ask(question: string, opts?: DialogPromptOptions): Promise<boolean> {
  if (typeof window === "undefined") return false;
  const title = opts?.title ?? "Confirm";
  return window.confirm(`${title}\n\n${question}`);
}

export async function message(msg: string, opts?: DialogPromptOptions): Promise<void> {
  if (typeof window === "undefined") return;
  const title = opts?.title ?? "Notice";
  window.alert(`${title}\n\n${msg}`);
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

export async function requestPermission(): Promise<"granted" | "denied" | "default"> {
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
export type GlassMaterialVariant = (typeof GlassMaterialVariant)[keyof typeof GlassMaterialVariant];

export async function isGlassSupported(): Promise<boolean> {
  return false;
}

export async function setLiquidGlassEffect(_opts?: unknown): Promise<void> {
  console.debug("[mock setLiquidGlassEffect]", _opts);
}
