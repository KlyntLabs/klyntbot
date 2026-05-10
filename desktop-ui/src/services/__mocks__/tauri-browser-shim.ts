// Browser-mode IPC shim: makes the UI work in plain Chrome at :1420 by
// installing `window.__TAURI_INTERNALS__` BEFORE `@tauri-apps/api/*` runs.
// `invoke()` proxies to the dev HTTP server at :3456; `listen()` (which goes
// through `transformCallback` + `invoke("plugin:event|listen")`) is bridged to
// `window` CustomEvents — `BrainEventBridge` already turns SSE frames into
// matching CustomEvents, so realtime UI works once both halves are in place.
//
// In the real Tauri webview, `__TAURI_INTERNALS__` is already defined → this
// file short-circuits and the native runtime takes over.

const DEV_BASE = "http://127.0.0.1:3456";

type Internals = {
  invoke: <T>(cmd: string, args?: Record<string, unknown>, opts?: unknown) => Promise<T>;
  transformCallback: (handler: (event: unknown) => void, once?: boolean) => number;
  unregisterListener: (event: string, eventId: number) => Promise<void>;
  convertFileSrc: (filePath: string, protocol?: string) => string;
  metadata: { currentWindow: { label: string } };
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: Internals;
  }
}

if (typeof window !== "undefined" && !window.__TAURI_INTERNALS__) {
  const callbacks = new Map<number, (e: unknown) => void>();
  const eventListeners = new Map<number, { event: string; handler: EventListener }>();
  let nextId = 1;

  // Plugin commands we either bridge specially or no-op. Routing them to the
  // dev server would 404 (they're handled by Tauri's internal plugin runtime,
  // not by the dev_server router).
  const handlePluginCommand = (cmd: string, args?: Record<string, unknown>): unknown => {
    if (cmd === "plugin:event|listen") {
      const event = args?.event as string;
      const handlerId = args?.handler as number;
      const cb = callbacks.get(handlerId);
      if (cb) {
        const wrapped = (e: Event) => {
          const ce = e as CustomEvent;
          // Tauri delivers `{ event, id, payload }` to the transformed callback.
          cb({ event, id: handlerId, payload: ce.detail });
        };
        window.addEventListener(event, wrapped);
        eventListeners.set(handlerId, { event, handler: wrapped });
      }
      return handlerId;
    }
    if (cmd === "plugin:event|unlisten") {
      const eventId = args?.eventId as number;
      const entry = eventListeners.get(eventId);
      if (entry) {
        window.removeEventListener(entry.event, entry.handler);
        eventListeners.delete(eventId);
        callbacks.delete(eventId);
      }
      return undefined;
    }
    if (cmd === "plugin:event|emit" || cmd === "plugin:event|emit_to") {
      // Best-effort: also dispatch locally so any listener in this window picks it up.
      const event = args?.event as string;
      const payload = args?.payload;
      if (event) window.dispatchEvent(new CustomEvent(event, { detail: payload }));
      return undefined;
    }
    // Browser-mode fallbacks for the @tauri-apps/plugin-dialog commands.
    // Real Tauri handles these natively; in Chrome we surface
    // prompt/confirm/alert so the workspace + git lifecycles stay exercisable
    // end-to-end without the native runtime.
    if (cmd === "plugin:dialog|open") {
      const opts = (args?.options ?? args ?? {}) as Record<string, unknown>;
      const directory = Boolean(opts.directory);
      const multiple = Boolean(opts.multiple);
      const title = typeof opts.title === "string" ? opts.title : "Select";
      const placeholder = directory ? "/path/to/folder" : "/path/to/file";
      const hint = multiple
        ? `${title}\nEnter one or more absolute ${directory ? "folders" : "files"} (comma-separated).`
        : `${title}\n${placeholder}`;
      const defaultPath = typeof opts.defaultPath === "string" ? opts.defaultPath : "";
      const raw = window.prompt(hint, defaultPath);
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
    if (cmd === "plugin:dialog|save") {
      const opts = (args?.options ?? args ?? {}) as Record<string, unknown>;
      const title = typeof opts.title === "string" ? opts.title : "Save file";
      const defaultPath = typeof opts.defaultPath === "string" ? opts.defaultPath : "";
      const raw = window.prompt(`${title}\n/path/to/file`, defaultPath);
      if (raw == null) return null;
      const trimmed = raw.trim();
      return trimmed === "" ? null : trimmed;
    }
    if (cmd === "plugin:dialog|ask") {
      const msg = typeof args?.message === "string" ? args.message : "Confirm?";
      const opts = (args?.options ?? {}) as Record<string, unknown>;
      const title = typeof opts.title === "string" ? opts.title : "Confirm";
      return window.confirm(`${title}\n\n${msg}`);
    }
    if (cmd === "plugin:dialog|message") {
      const msg = typeof args?.message === "string" ? args.message : "";
      const opts = (args?.options ?? {}) as Record<string, unknown>;
      const title = typeof opts.title === "string" ? opts.title : "Notice";
      window.alert(`${title}\n\n${msg}`);
      return undefined;
    }
    // Window/menu/etc. plugins — silently no-op so the UI doesn't crash.
    return undefined;
  };

  const browserInvoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    if (cmd.startsWith("plugin:")) {
      return handlePluginCommand(cmd, args) as T;
    }
    let res: Response;
    try {
      res = await fetch(`${DEV_BASE}/api/${cmd}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(args ?? {}),
      });
    } catch (e) {
      throw new Error(`[browser-shim] network error for ${cmd}: ${(e as Error).message}`);
    }
    if (!res.ok) {
      let body: unknown;
      try {
        body = await res.json();
      } catch {
        body = await res.text();
      }
      const msg =
        typeof body === "object" && body && "message" in body
          ? String((body as { message: unknown }).message)
          : typeof body === "string"
            ? body
            : JSON.stringify(body);
      throw new Error(`${cmd} → HTTP ${res.status}: ${msg}`);
    }
    if (res.status === 204) return undefined as T;
    const text = await res.text();
    if (!text) return undefined as T;
    try {
      return JSON.parse(text) as T;
    } catch {
      return text as unknown as T;
    }
  };

  window.__TAURI_INTERNALS__ = {
    invoke: browserInvoke,
    transformCallback: (handler, _once) => {
      const id = nextId++;
      callbacks.set(id, handler as (e: unknown) => void);
      return id;
    },
    unregisterListener: async (_event, eventId) => {
      const entry = eventListeners.get(eventId);
      if (entry) {
        window.removeEventListener(entry.event, entry.handler);
        eventListeners.delete(eventId);
      }
      callbacks.delete(eventId);
    },
    convertFileSrc: (filePath, protocol = "asset") =>
      `${protocol}://localhost/${encodeURIComponent(filePath)}`,
    metadata: { currentWindow: { label: "main" } },
  };

  (window as unknown as { __TAURI_BROWSER_SHIM__?: boolean }).__TAURI_BROWSER_SHIM__ = true;

  // Event plugin uses its own internals global. `_unlisten` in
  // @tauri-apps/api/event reads `window.__TAURI_EVENT_PLUGIN_INTERNALS__`.
  type EventPluginInternals = {
    unregisterListener: (event: string, eventId: number) => void;
  };
  (
    window as unknown as { __TAURI_EVENT_PLUGIN_INTERNALS__?: EventPluginInternals }
  ).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (_event, eventId) => {
      const entry = eventListeners.get(eventId);
      if (entry) {
        window.removeEventListener(entry.event, entry.handler);
        eventListeners.delete(eventId);
      }
      callbacks.delete(eventId);
    },
  };

  // eslint-disable-next-line no-console
  console.info(`[browser-shim] installed __TAURI_INTERNALS__ → ${DEV_BASE}`);
}

export {};
